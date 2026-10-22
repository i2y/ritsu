//! Every code of the ledger (src/codes.rs) that is printed has an example that gives it, in English
//! names and in Japanese ones, checked as `ritsu kaname check` checks it (E202's as the binary of
//! kaname's own crate does); and `docs/codes.md` and `docs/codes.ja.md` are what `kaname explain
//! --all --format markdown` prints.

mod common;

use kaname::check::Run;
use kaname::suite::Suite;
use ritsu_base::ledger::{self, Repro};
use ritsu_base::text::Lang;

#[test]
fn every_example_gives_its_code() {
    let scratch = ritsu_testkit::TempDir::new("codes");
    let failures = ledger::check_every(&kaname::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.kn");
        let suite = if e.code == "E202" { Suite::default() } else { common::joined() };
        let o = Run::new(suite).file(path.to_str().unwrap()).unwrap();
        let codes: Vec<String> = o.diags.iter().map(|d| d.code.to_string()).collect();
        if !codes.iter().any(|c| c == e.code) {
            let text: String = o.diags.iter().map(|d| d.render(Lang::En)).collect();
            eprintln!("{}:\n{text}", e.code);
        }
        codes
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// An example prints its code and no other: what the reader sees is about that code alone.
#[test]
fn every_example_gives_its_code_alone() {
    let scratch = ritsu_testkit::TempDir::new("codes-alone");
    let mut noisy = Vec::new();
    ledger::check_every(&kaname::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.kn");
        let suite = if e.code == "E202" { Suite::default() } else { common::joined() };
        let o = Run::new(suite).file(path.to_str().unwrap()).unwrap();
        let codes: Vec<String> = o.diags.iter().map(|d| d.code.to_string()).collect();
        if codes.iter().any(|c| c != e.code) {
            noisy.push(format!("{}: {codes:?}", e.code));
        }
        codes
    });
    assert!(noisy.is_empty(), "{}", noisy.join("\n"));
}

#[test]
fn the_ledger_has_each_code_once() {
    let l = kaname::codes::ledger();
    assert!(ledger::duplicates(&l).is_empty(), "{:?}", ledger::duplicates(&l));
    // DESIGN 12: E001–E007, E101–E115, E201–E204, E301–W303, E401, E402, W501, E601, E602, W901
    assert_eq!(l.entries.len(), 38);
    // every code is printed by now, and has its example
    let later: Vec<&str> = l.entries.iter().filter(|e| matches!(e.repro, Repro::Later)).map(|e| e.code).collect();
    assert!(later.is_empty(), "{later:?}");
}

#[test]
fn the_documents_of_the_codes_are_what_explain_prints() {
    let l = kaname::codes::ledger();
    ritsu_testkit::golden("docs/codes.md", &l.render_markdown(Lang::En));
    ritsu_testkit::golden("docs/codes.ja.md", &l.render_markdown(Lang::Ja));
}

/// What `kaname check` says of each example, in English and in Japanese: the text a person reads,
/// held to `tests/golden/codes/` (`KANAME_BLESS=1` writes them again). The directory the example is
/// laid out in is left out of the paths.
#[test]
fn the_examples_say_what_the_golden_files_hold() {
    let scratch = ritsu_testkit::TempDir::new("codes-golden");
    let mut failures = Vec::new();
    ledger::check_every(&kaname::codes::ledger(), scratch.path(), |e, dir| {
        let path = dir.join("example.kn");
        let suite = if e.code == "E202" { Suite::default() } else { common::joined() };
        let o = Run::new(suite).file(path.to_str().unwrap()).unwrap();
        let ja = e.repro_ja.is_none() && dir.file_name().is_some_and(|n| n.to_string_lossy().ends_with("-ja"));
        let lang = if ja { Lang::Ja } else { Lang::En };
        let prefix = format!("{}/", dir.display());
        let said = kaname::check::render(&o, lang).replace(&prefix, "");
        let name = format!("tests/golden/codes/{}.{}.txt", e.code, lang.code());
        if let Err(x) = ritsu_testkit::golden::check(std::path::Path::new(&name), &said) {
            failures.push(x);
        }
        o.diags.iter().map(|d| d.code.to_string()).collect()
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// What the codes say of the values of an enum that have fields, a `match` with `else` and literals,
/// and a record made from another (DESIGN 23): each case, in English names and in Japanese ones,
/// gives its code alone, and the text a person reads is held to `tests/golden/codes/sums.*.txt`.
#[test]
fn the_cases_of_values_with_fields_say_their_codes() {
    let order_en = "enum Order\n  draft\n  placed\n    on : date  range >=2026-01-01 <=2026-12-31\n\n";
    let order_ja = "enum 注文(Order)\n  下書き(draft)\n  確定(placed)\n    日(on) : date  range >=2026-01-01 <=2026-12-31\n\n";
    let tier_en = "enum tier = basic | gold\n\n";
    let tier_ja = "enum 区分(tier) = 一般(basic) | 金(gold)\n\n";
    let cases: Vec<(&str, String, String)> = vec![
        // `else` that takes no value, an arm after it, a `match` by literals with none
        ("E106", format!("{tier_en}fn points(t: tier) -> number\n  match t\n    basic, gold => 1\n    else => 2\n"), format!("{tier_ja}fn 点数(points)(区分(t): 区分) -> number\n  match 区分\n    一般, 金 => 1\n    else => 2\n")),
        ("E106", format!("{tier_en}fn points(t: tier) -> number\n  match t\n    else => 1\n    gold => 2\n"), format!("{tier_ja}fn 点数(points)(区分(t): 区分) -> number\n  match 区分\n    else => 1\n    金 => 2\n")),
        ("E106", "fn grade(code: string) -> number\n  match code\n    \"A\" => 3\n    \"B\" => 2\n".into(), "fn 点数(grade)(評価(code): string) -> number\n  match 評価\n    \"A\" => 3\n    \"B\" => 2\n".into()),
        // a name for a value with no fields
        ("E106", format!("{tier_en}fn points(t: tier) -> number\n  match t\n    gold g => 1\n    else => 2\n"), format!("{tier_ja}fn 点数(points)(区分(t): 区分) -> number\n  match 区分\n    金 g => 1\n    else => 2\n")),
        // a field read with `.`, braces on a value with no fields, a value with fields alone
        ("E105", format!("{order_en}fn day(o: Order) -> date\n  o.on\n"), format!("{order_ja}fn 日付(day)(現在の注文(o): 注文) -> date\n  現在の注文.日\n")),
        ("E105", format!("{order_en}fn start() -> Order\n  Order.draft {{ on: 2026-01-01 }}\n"), format!("{order_ja}fn 始め(start)() -> 注文\n  注文.下書き {{ 日: 2026-01-01 }}\n")),
        ("E107", format!("{order_en}fn start() -> Order\n  Order.placed\n"), format!("{order_ja}fn 始め(start)() -> 注文\n  注文.確定\n")),
        // the type of a value no enum has
        ("E101", format!("{order_en}fn day(p: Order.shipped) -> number\n  1\n"), format!("{order_ja}fn 日付(day)(発送(p): 注文.発送済み) -> number\n  1\n")),
        // a value made from one of another type
        ("E102", format!("{order_en}fn moved(o: Order) -> Order\n  Order.placed {{ ..o, on: 2026-03-01 }}\n"), format!("{order_ja}fn 日の変更(moved)(現在の注文(o): 注文) -> 注文\n  注文.確定 {{ ..現在の注文, 日: 2026-03-01 }}\n")),
        // a literal of an arm past int64
        ("E104", "fn size(n: number range >=0 <=9) -> string\n  match n\n    9_223_372_036_854_775_808 => \"big\"\n    else => \"small\"\n".into(), "fn 大きさ(size)(数(n): number range >=0 <=9) -> string\n  match 数\n    9_223_372_036_854_775_808 => \"大きい\"\n    else => \"小さい\"\n".into()),
        // a rate of a value's field with no step, an enum every value of which holds another
        ("E108", "enum Fee\n  flat\n    share : rate\n  none_due\n".into(), "enum 手数料(Fee)\n  定率(flat)\n    率(share) : rate\n  なし(none_due)\n".into()),
        ("E108", "enum Tree\n  node\n    left : Tree\n".into(), "enum 木(Tree)\n  節(node)\n    左(left) : 木\n".into()),
        // the names of the generated code: a value's type and a record, the field `kind`
        ("E006", format!("{order_en}record OrderPlaced\n  on : date\n"), format!("{order_ja}record 確定した注文(OrderPlaced)\n  日(on) : date\n")),
        ("E007", "enum Order\n  draft\n    kind : string\n  cancelled\n".into(), "enum 注文(Order)\n  下書き(draft)\n    種類(kind) : string\n  取り消し(cancelled)\n".into()),
        // a record and a value of one name, where the enum goes
        ("E110", format!("{order_en}record placed\n  on : date\n\nfn start() -> Order\n  placed {{ on: 2026-03-01 }}\n"), format!("{order_ja}record 確定(placed_record)\n  日(on) : date\n\nfn 始め(start)() -> 注文\n  確定 {{ 日: 2026-03-01 }}\n")),
    ];
    say_their_codes("sums", &cases);
}

/// What the codes say of dicts, sets and the builtins of lists (DESIGN 24): a key of a type that
/// cannot be one, a builtin written in the form it does not take, a dict literal's key given twice,
/// an unrounded value a dict holds, the divisor of `mod`, the name `numbers`, a `count` on what holds
/// no elements, `[:]` alone. Each case, in English names and in Japanese ones, gives its code alone,
/// and the text a person reads is held to `tests/golden/codes/dicts.*.txt`.
#[test]
fn the_cases_of_dicts_sets_and_lists_say_their_codes() {
    let order_en = "record Order\n  id : string\n  lines : list[number]\n\n";
    let order_ja = "record 注文(Order)\n  番号(id) : string\n  明細(lines) : list[number]\n\n";
    let cases: Vec<(&str, String, String)> = vec![
        // a key of an order that is a list, a member of a set that is a record
        ("E114", format!("{order_en}fn ordered(xs: list[Order]) -> list[Order]\n  xs.sort_by(x => x.lines)\n"), format!("{order_ja}fn 並べる(ordered)(注文の一覧(xs): list[注文]) -> list[注文]\n  注文の一覧.sort_by(x => x.明細)\n")),
        ("E114", format!("{order_en}fn kinds(xs: list[Order]) -> number\n  xs.to_set().count()\n"), format!("{order_ja}fn 種類(kinds)(注文の一覧(xs): list[注文]) -> number\n  注文の一覧.to_set().count()\n")),
        // a builtin written after a value, called as a function; one called as a function, written after a value
        ("E101", "fn names(prices: dict[string, number]) -> list[string]\n  keys(prices)\n".into(), "fn 品名(names)(価格表(prices): dict[string, number]) -> list[string]\n  keys(価格表)\n".into()),
        ("E101", "fn days(start: number range >=1 <=31) -> list[number]\n  start.numbers(31)\n".into(), "fn 日の一覧(days)(初日(start): number range >=1 <=31) -> list[number]\n  初日.numbers(31)\n".into()),
        // a constant key twice in a dict literal, the arguments after the value
        ("E107", "fn prices() -> dict[string, money[USDc]]\n  [\"tea\": 300USDc, \"tea\": 350USDc]\n".into(), "fn 価格表(prices)() -> dict[string, money[USDc]]\n  [\"紅茶\": 300USDc, \"紅茶\": 350USDc]\n".into()),
        ("E107", "fn head(xs: list[number]) -> list[number]\n  xs.take()\n".into(), "fn 先頭(head)(数の一覧(xs): list[number]) -> list[number]\n  数の一覧.take()\n".into()),
        // an unrounded value as a value of a dict
        ("E103", "fn taxes(prices: dict[string, money[USDc]]) -> dict[string, money[USDc]]\n  prices.map_values(p => p * 8%)\n".into(), "fn 税(taxes)(価格表(prices): dict[string, money[USDc]]) -> dict[string, money[USDc]]\n  価格表.map_values(p => p * 8%)\n".into()),
        // the divisor of `mod`: a whole number, a constant or a value
        ("E109", "fn bucket(n: number range >=0 <=99, size: money[USDc] range >=1USDc <=10USDc) -> number\n  mod(n, size)\n".into(), "fn 組(bucket)(番号(n): number range >=0 <=99, 幅(size): money[USDc] range >=1USDc <=10USDc) -> number\n  mod(番号, 幅)\n".into()),
        // `numbers` is a builtin's name; a `count` on a number; `[:]` alone
        ("E006", "fn numbers() -> number\n  1\n".into(), "fn numbers() -> number\n  1\n".into()),
        ("E108", "fn total(n: number count >=1) -> number\n  n\n".into(), "fn 合計(total)(数(n): number count >=1) -> number\n  数\n".into()),
        ("E102", "fn empty() -> number\n  let d = [:]\n  d.count()\n".into(), "fn 空(empty)() -> number\n  let 辞書 = [:]\n  辞書.count()\n".into()),
        ("E102", "fn names(n: number range >=0 <=9) -> list[string]\n  n.keys()\n".into(), "fn 品名(names)(数(n): number range >=0 <=9) -> list[string]\n  数.keys()\n".into()),
    ];
    say_their_codes("dicts", &cases);
}

/// What the codes say of generics, results and `try` (DESIGN 26): a type argument that cannot be
/// worked out, given in the wrong number, or of a type whose values may be none; a type argument made
/// from a type parameter, given in a ring of calls by a call and by a function written as a value; a
/// value of a type parameter ordered with no `key`, added to, given a range; `try` under `let y =`,
/// with another error, on what is neither a result nor a value that may be none; the words `try` and
/// `ok`; an unrounded value given to a type parameter. Each case, in English names and in Japanese
/// ones, gives its code alone, and the text a person reads is held to
/// `tests/golden/codes/generics.*.txt`.
#[test]
fn the_cases_of_generics_and_results_say_their_codes() {
    let first_en = "fn first_or[T](xs: list[T], fallback: T) -> T\n  match xs.first()\n    none => fallback\n    some x => x\n\n";
    let first_ja = "fn 先頭か(first_or)[T](一覧(xs): list[T], 代わり(fallback): T) -> T\n  match 一覧.first()\n    none => 代わり\n    some x => x\n\n";
    let page_en = "record Page[T]\n  items : list[T]\n\nfn empty_page[T]() -> Page[T]\n  Page { items: [] }\n\n";
    let page_ja = "record 頁(Page)[T]\n  項目(items) : list[T]\n\nfn 空の頁(empty_page)[T]() -> 頁[T]\n  頁 { 項目: [] }\n\n";
    let half_en = "fn half(n: number range >=0 <=9) -> result[number, string]\n  if n = 0 then err(\"zero\") else ok(n)\n\n";
    let half_ja = "fn 半分(half)(数(n): number range >=0 <=9) -> result[number, string]\n  if 数 = 0 then err(\"ゼロ\") else ok(数)\n\n";
    let cases: Vec<(&str, String, String)> = vec![
        // a type parameter no argument and no place says; two type arguments for one; `number?`
        ("E112", format!("{page_en}fn size() -> number\n  let p = empty_page()\n  p.items.count()\n"), format!("{page_ja}fn 大きさ(size)() -> number\n  let 頁の値 = 空の頁()\n  頁の値.項目.count()\n")),
        ("E112", format!("{page_en}fn size(p: Page[number, string]) -> number\n  p.items.count()\n"), format!("{page_ja}fn 大きさ(size)(頁の値(p): 頁[number, string]) -> number\n  頁の値.項目.count()\n")),
        ("E112", format!("{first_en}fn head(xs: list[number?]) -> number?\n  first_or(xs: xs, fallback: none)\n"), format!("{first_ja}fn 先頭(head)(一覧(xs): list[number?]) -> number?\n  先頭か(一覧: 一覧, 代わり: none)\n")),
        // a type argument made from a type parameter, given in a ring of calls: by a call, and by a
        // function written as a value
        (
            "E112",
            "fn wrap_deep[T](x: T, n: number range >=0 <=3) -> number\n  if n = 0 then 0 else wrap_deep(x: [x], n: n - 1)\n".into(),
            "fn 包んで潜る(wrap_deep)[T](値(x): T, 数(n): number range >=0 <=3) -> number\n  if 数 = 0 then 0 else 包んで潜る(値: [値], 数: 数 - 1)\n".into(),
        ),
        (
            "E112",
            "fn sizes[T](xs: list[T]) -> list[number]\n  [[xs]].map(lengths)\n\nfn lengths[T](xs: list[T]) -> number\n  sizes(xs: xs).count()\n".into(),
            "fn 大きさの一覧(sizes)[T](一覧(xs): list[T]) -> list[number]\n  [[一覧]].map(長さ)\n\nfn 長さ(lengths)[T](一覧(xs): list[T]) -> number\n  大きさの一覧(一覧: 一覧).count()\n".into(),
        ),
        // a value of a type parameter ordered with no `key`, added to, given a range
        ("E114", "fn before[T](a: T, b: T) -> bool\n  a < b\n".into(), "fn 前か(before)[T](左(a): T, 右(b): T) -> bool\n  左 < 右\n".into()),
        ("E105", "fn twice[T](x: T) -> T\n  x + x\n".into(), "fn 二倍(twice)[T](値(x): T) -> T\n  値 + 値\n".into()),
        ("E108", "fn same[T](x: T range >=1) -> T\n  x\n".into(), "fn 同じ(same)[T](値(x): T range >=1) -> T\n  値\n".into()),
        // `try` under `let y =`, with another error, on a value that is neither
        ("E115", format!("{half_en}fn next(n: number range >=0 <=9) -> result[number, string]\n  let b =\n    let h = try half(n: n)\n    h\n  ok(b)\n"), format!("{half_ja}fn 次(next)(数(n): number range >=0 <=9) -> result[number, string]\n  let b =\n    let h = try 半分(数: 数)\n    h\n  ok(b)\n")),
        ("E115", format!("{half_en}fn next(n: number range >=0 <=9) -> result[number, list[string]]\n  let h = try half(n: n)\n  ok(h)\n"), format!("{half_ja}fn 次(next)(数(n): number range >=0 <=9) -> result[number, list[string]]\n  let h = try 半分(数: 数)\n  ok(h)\n")),
        ("E115", "fn same(n: number range >=0 <=9) -> number?\n  let m = try n\n  m\n".into(), "fn 同じ(same)(数(n): number range >=0 <=9) -> number?\n  let m = try 数\n  m\n".into()),
        // the word `try`; a function named as the builtin `ok`
        ("E002", "fn f(try: number) -> number\n  try\n".into(), "fn 関数(f)(try: number) -> number\n  try\n".into()),
        ("E006", "fn ok(x: number) -> number\n  x\n".into(), "fn ok(x: number) -> number\n  x\n".into()),
        // an unrounded value given to a type parameter
        ("E103", format!("{first_en}fn fee(a: money[USDc] range >=1USDc <=9USDc) -> money[USDc]\n  first_or(xs: [a * 3%], fallback: a)\n"), format!("{first_ja}fn 手数料(fee)(金額(a): money[USDc] range >=1USDc <=9USDc) -> money[USDc]\n  先頭か(一覧: [金額 * 3%], 代わり: 金額)\n")),
    ];
    say_their_codes("generics", &cases);
}

/// What the codes say of function values and `private` (DESIGN 27, 28.2): a function value where none
/// goes, a function that takes functions as a value, a lambda whose parameters' types nothing says, a
/// function type as a type argument, function types that do not agree, a function value called with
/// names or with the wrong number of arguments, a builtin as a value, a function given that does more
/// than the effect its parameter's type writes, and a `private` type in what is not `private`. Each
/// case, in English names and in Japanese ones, gives its code alone, and the text a person reads is
/// held to `tests/golden/codes/hof.*.txt`.
#[test]
fn the_cases_of_function_values_say_their_codes() {
    let each_en = "fn apply_each[T](xs: list[T], f: (T) -> T) -> list[T]\n  xs.map(x => f(x))\n\n";
    let each_ja = "fn それぞれに(apply_each)[T](一覧(xs): list[T], 関数(f): (T) -> T) -> list[T]\n  一覧.map(x => 関数(x))\n\n";
    let twice_en = "fn twice(f: (number) -> number, x: number range >=0 <=9) -> number\n  f(f(x))\n\n";
    let twice_ja = "fn 二度(twice)(関数(f): (number) -> number, 数(x): number range >=0 <=9) -> number\n  関数(関数(数))\n\n";
    let within_en = "fn all_within(xs: list[number] count <=9, test: (number) -> bool total) -> bool\n  xs.all(x => test(x))\n\n";
    let within_ja = "fn すべて満たす(all_within)(数の一覧(xs): list[number] count <=9, 条件(test): (number) -> bool total) -> bool\n  数の一覧.all(x => 条件(x))\n\n";
    let cases: Vec<(&str, String, String)> = vec![
        // a function value as an answer, a field, an element, a side of `=`; a function that takes
        // functions as a value
        ("E113", "fn adder(n: number range >=0 <=9) -> (number) -> number\n  x => x + n\n".into(), "fn 足す関数(adder)(数(n): number range >=0 <=9) -> (number) -> number\n  x => x + 数\n".into()),
        ("E113", "record Step\n  run : (number) -> number\n".into(), "record 手順(Step)\n  実行(run) : (number) -> number\n".into()),
        ("E113", "fn steps() -> number\n  let fs = [(y: number) => y + 1]\n  1\n".into(), "fn 手順(steps)() -> number\n  let 関数の一覧 = [(y: number) => y + 1]\n  1\n".into()),
        ("E113", "fn same(f: (number) -> number, g: (number) -> number) -> bool\n  f = g\n".into(), "fn 同じか(same)(左(f): (number) -> number, 右(g): (number) -> number) -> bool\n  左 = 右\n".into()),
        ("E113", format!("{each_en}fn held() -> number\n  let g = apply_each\n  1\n"), format!("{each_ja}fn 持つ(held)() -> number\n  let g = それぞれに\n  1\n")),
        // the types of a lambda's parameters that nothing says; a function type as a type argument
        ("E112", "fn one() -> number\n  let twice = x => x * 2\n  1\n".into(), "fn 一(one)() -> number\n  let 倍 = x => x * 2\n  1\n".into()),
        ("E112", "fn id[T](x: T) -> T\n  x\n\nfn one() -> number\n  let g = id(x: (y: number) => y + 1)\n  1\n".into(), "fn 同じ値(id)[T](値(x): T) -> T\n  値\n\nfn 一(one)() -> number\n  let g = 同じ値(値: (y: number) => y + 1)\n  1\n".into()),
        // function types that do not agree
        ("E102", format!("{twice_en}fn four() -> number\n  twice(f: (s: string) => 1, x: 1)\n"), format!("{twice_ja}fn 四(four)() -> number\n  二度(関数: (s: string) => 1, 数: 1)\n")),
        ("E102", format!("{twice_en}fn four() -> number\n  twice(f: 3, x: 1)\n"), format!("{twice_ja}fn 四(four)() -> number\n  二度(関数: 3, 数: 1)\n")),
        // a function value called with names, with the wrong number of arguments
        ("E107", "fn apply(f: (number) -> number, x: number range >=0 <=9) -> number\n  f(x: x)\n".into(), "fn 当てる(apply)(関数(f): (number) -> number, 数(x): number range >=0 <=9) -> number\n  関数(x: 数)\n".into()),
        ("E107", "fn apply(f: (number) -> number, x: number range >=0 <=9) -> number\n  f(x, x)\n".into(), "fn 当てる(apply)(関数(f): (number) -> number, 数(x): number range >=0 <=9) -> number\n  関数(数, 数)\n".into()),
        // a builtin as a value
        ("E109", "fn roots(xs: list[number] count <=9 range >=0 <=99) -> list[number]\n  xs.map(isqrt)\n".into(), "fn 平方根(roots)(数の一覧(xs): list[number] count <=9 range >=0 <=99) -> list[number]\n  数の一覧.map(isqrt)\n".into()),
        // a function given that does more than its parameter's type writes: a lambda, a function by its name
        ("E402", format!("{within_en}fn grows(xs: list[number] count <=9) -> bool\n  all_within(xs: xs, test: x => x + 1 > x)\n"), format!("{within_ja}fn 増える(grows)(数の一覧(xs): list[number] count <=9) -> bool\n  すべて満たす(数の一覧: 数の一覧, 条件: x => x + 1 > x)\n")),
        ("E402", format!("{within_en}fn next(n: number) -> bool\n  n + 1 > n\n\nfn grows(xs: list[number] count <=9) -> bool\n  all_within(xs: xs, test: next)\n"), format!("{within_ja}fn 次が大きい(next)(数(n): number) -> bool\n  数 + 1 > 数\n\nfn 増える(grows)(数の一覧(xs): list[number] count <=9) -> bool\n  すべて満たす(数の一覧: 数の一覧, 条件: 次が大きい)\n")),
        // a `private` type in what is not `private`
        ("E108", "private record Bounds\n  low : number  range >=0 <=9\n\nfn low(b: Bounds) -> number\n  b.low\n".into(), "private record 帯(Bounds)\n  下限(low) : number  range >=0 <=9\n\nfn 下限の値(low)(区間(b): 帯) -> number\n  区間.下限\n".into()),
    ];
    say_their_codes("hof", &cases);
}

/// What the codes say of the types that hold themselves (DESIGN 28.1): a record, a record through a
/// list that cannot be empty and an enum whose every value holds another of it (none of them can be
/// made), and a generic record that gives itself a type argument made from its type parameter. Each
/// case, in English names and in Japanese ones, gives its code alone, and the text a person reads is
/// held to `tests/golden/codes/trees.*.txt`.
#[test]
fn the_cases_of_types_that_hold_themselves_say_their_codes() {
    let cases: Vec<(&str, String, String)> = vec![
        ("E108", "record Node\n  value : number  range >=0 <=9\n  next  : Node\n".into(), "record 節点(Node)\n  値(value) : number  range >=0 <=9\n  次(next)  : 節点\n".into()),
        ("E108", "record Category\n  name     : string\n  children : list[Category]  count >=1\n".into(), "record 分類(Category)\n  名前(name)   : string\n  子(children) : list[分類]  count >=1\n".into()),
        ("E108", "enum Expr\n  add\n    left  : Expr\n    right : Expr\n  neg\n    inner : Expr\n".into(), "enum 式(Expr)\n  和(add)\n    左(left)  : 式\n    右(right) : 式\n  反転(neg)\n    中身(inner) : 式\n".into()),
        ("E108", "record Wrap[T]\n  value : T\n  inner : Wrap[list[T]]?\n".into(), "record 包み(Wrap)[T]\n  値(value)   : T\n  中身(inner) : 包み[list[T]]?\n".into()),
    ];
    say_their_codes("trees", &cases);
}

/// Each case, its English module and its Japanese one, gives its code alone; what `check` prints of
/// them is held to `tests/golden/codes/<name>.{en,ja}.txt`.
fn say_their_codes(name: &str, cases: &[(&str, String, String)]) {
    let scratch = ritsu_testkit::TempDir::new(&format!("codes-{name}"));
    let (mut en, mut ja) = (String::new(), String::new());
    let mut wrong = Vec::new();
    for (i, (code, src_en, src_ja)) in cases.iter().enumerate() {
        for (lang, head, src, out) in [(Lang::En, "module example v1\n\n", src_en, &mut en), (Lang::Ja, "module 例(example) v1\n\n", src_ja, &mut ja)] {
            let dir = scratch.path().join(format!("{i}-{}", lang.code()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("example.kn");
            std::fs::write(&path, format!("{head}{src}")).unwrap();
            let o = Run::new(common::joined()).file(path.to_str().unwrap()).unwrap();
            let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
            if codes != [*code] {
                wrong.push(format!("{code} ({lang:?}): {codes:?}\n{}", kaname::check::render(&o, Lang::En)));
            }
            let prefix = format!("{}/", dir.display());
            out.push_str(&format!("── {code}\n{}\n", kaname::check::render(&o, lang).replace(&prefix, "")));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    ritsu_testkit::golden(&format!("tests/golden/codes/{name}.en.txt"), &en);
    ritsu_testkit::golden(&format!("tests/golden/codes/{name}.ja.txt"), &ja);
}
