//! `import proto` (§15.59): an enum whose value set belongs to a service contract.
//!
//! The line this draws is a loop with two gates, and both have to hold. The contract gains a
//! value and the rule stops (E032) — this gate fires wherever the `.proto` was changed, a
//! change that is compatible on the wire and invisible to everything else.
//! The value is then written into the rule and it stops again (E033) until somebody says what
//! the table does with it, because a table with a default row would otherwise pass the
//! completeness check and quietly charge the new tier the default amount.

use std::path::PathBuf;
use std::process::Command;
use ritsu_testkit::TempDir;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("proto-{tag}"));
    let d = t.path().to_path_buf();
    (t, d)
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .env("RULEC_LANG", "ja")
        .args(args)
        .args(if args.contains(&"--lang") { &[][..] } else { &["--lang", "ja"][..] })
        .output()
        .expect("rulec を起動できない");
    let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&o.stderr));
    (o.status.code().unwrap_or(-1), s)
}

const PROTO: &str = "\
syntax = \"proto3\";

package shop.v1;

// The member tier the fee table branches on.
enum MemberTier {
  MEMBER_TIER_UNSPECIFIED = 0;
  MEMBER_TIER_BASIC = 1;
  MEMBER_TIER_GOLD = 2;
}

message FeeRequest {
  MemberTier tier = 1;
  int64 weight_g = 2;
}
";

/// A rule whose enum is bound to `MemberTier`. `values` is written into the enum line and
/// `rows` into the table, so a test states only what it is about.
fn rule(values: &str, rows: &str) -> String {
    format!(
        "rule 配送料金(delivery_fee) v1\n\n\
         import proto \"order.proto\" MemberTier -> 会員区分\n\
         enum 会員区分(tier) = {values}\n\n\
         inputs\n  会員(m) : 会員区分\n\n\
         outputs\n  送料(fee) : money[円, incl_tax]  round up(10円)\n\n\
         table 送料表(fee_table)\npolicy first\n\
         | 会員 | -> 送料(fee) : money[円, incl_tax] |\n{rows}"
    )
}

/// Writes the pair and returns the path of the `.rule`.
fn pair(tag: &str, proto: &str, rule: &str) -> (TempDir, String) {
    let (t, d) = dir(tag);
    std::fs::write(d.join("order.proto"), proto).unwrap();
    let p = d.join("配送料金.rule");
    std::fs::write(&p, rule).unwrap();
    (t, p.to_string_lossy().into_owned())
}

#[test]
fn 列挙だけを読む() {
    let src = "\
syntax = \"proto3\";
// enum Decoy { DECOY_A = 0; }   ← a comment is not a declaration
message Order {
  /* nor is this */
  enum Status {
    STATUS_UNSPECIFIED = 0;
    STATUS_PAID = 1 [deprecated = true];
    reserved 2, 3;
    option allow_alias = true;
    STATUS_SHIPPED = 4;
  }
  Status status = 1;
  string note = 2;  // \"enum Fake { F = 0; }\" inside a string is text
}
";
    let es = rulec::proto::read("order.proto", src).expect("the file reads").enums;
    assert_eq!(es.len(), 1, "読めた列挙: {:?}", es.iter().map(|e| &e.name).collect::<Vec<_>>());
    assert_eq!(es[0].name, "Status");
    let names: Vec<&str> = es[0].values.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["STATUS_UNSPECIFIED", "STATUS_PAID", "STATUS_SHIPPED"]);
    // The prefix is the enum's own name and is not part of the value; the zero value is
    // proto3's "not set" and is not a value a table answers for.
    assert_eq!(es[0].aliases(), ["paid", "shipped"]);
}

#[test]
fn ゼロ値が_unspecified_でなければ落とさない() {
    // A file that puts a real value at 0 is unusual. Deciding on its behalf that it means
    // nothing is exactly the silence this tool exists to remove.
    let es = rulec::proto::read("tier.proto", "enum Tier { TIER_NONE = 0; TIER_ONE = 1; }").expect("the file reads").enums;
    assert_eq!(es[0].aliases(), ["none", "one"]);
}

#[test]
fn 一致していれば何も言わない() {
    let (_tmp, p) = pair("ok", PROTO, &rule("一般(basic) | ゴールド(gold) default", "| 一般 | 800円 |\n| - | 400円 |\n"));
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("ok"), "{out}");
}

#[test]
fn 契約に値が増えたら止まる() {
    let grown = PROTO.replace("  MEMBER_TIER_GOLD = 2;", "  MEMBER_TIER_GOLD = 2;\n  MEMBER_TIER_PLATINUM = 3;");
    let (_tmp, p) = pair("grown", &grown, &rule("一般(basic) | ゴールド(gold) default", "| 一般 | 800円 |\n| - | 400円 |\n"));
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E032"), "{out}");
    // The value is named, and so is the form to add — the name itself is the author's to
    // choose, because a proto carries no Japanese.
    assert!(out.contains("platinum"), "{out}");
    assert!(out.contains("<名前>(platinum)"), "{out}");
}

#[test]
fn 規則にだけある値も止まる() {
    let (_tmp, p) = pair(
        "extra",
        PROTO,
        &rule("一般(basic) | ゴールド(gold) default | 白金(platinum) default", "| 一般 | 800円 |\n| - | 400円 |\n"),
    );
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E032") && out.contains("platinum"), "{out}");
}

/// The case the feature exists for: the sets agree again, the table has a catch-all row, so
/// completeness passes — and the new tier would silently take the default amount.
#[test]
fn 既定の行があっても_決めていない値は止まる() {
    let grown = PROTO.replace("  MEMBER_TIER_GOLD = 2;", "  MEMBER_TIER_GOLD = 2;\n  MEMBER_TIER_PLATINUM = 3;");
    let r = rule("一般(basic) | ゴールド(gold) default | 白金(platinum)", "| 一般 | 800円 |\n| - | 400円 |\n");
    let (_tmp, p) = pair("undecided", &grown, &r);
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E033") && out.contains("白金"), "{out}");
    // W111 would have said the same thing and exited 0. That is the whole difference.
    assert!(!out.contains("W111"), "{out}");
}

#[test]
fn 行を足すか_default_を付ければ通る() {
    let grown = PROTO.replace("  MEMBER_TIER_GOLD = 2;", "  MEMBER_TIER_GOLD = 2;\n  MEMBER_TIER_PLATINUM = 3;");
    let values = "一般(basic) | ゴールド(gold) default | 白金(platinum)";
    let (_tmp, row) = pair("decided-row", &grown, &rule(values, "| 一般 | 800円 |\n| 白金 | 0円 |\n| - | 400円 |\n"));
    let (code, out) = run(&["check", &row]);
    assert_eq!(code, 0, "{out}");

    let (_tmp, marked) = pair(
        "decided-default",
        &grown,
        &rule("一般(basic) | ゴールド(gold) default | 白金(platinum) default", "| 一般 | 800円 |\n| - | 400円 |\n"),
    );
    let (code, out) = run(&["check", &marked]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn 読めないファイルと_無い列挙と_形の違う行をエラーにする() {
    let plain = rule("一般(basic) | ゴールド(gold) default", "| - | 400円 |\n");
    let (_tmp, missing) = pair("missing", PROTO, &plain.replace("order.proto", "nope.proto"));
    let (code, out) = run(&["check", &missing]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E013") && out.contains("nope.proto"), "{out}");

    let (_tmp, wrong) = pair("wrong-enum", PROTO, &plain.replace("MemberTier", "Tier"));
    let (code, out) = run(&["check", &wrong]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E013") && out.contains("MemberTier"), "そのファイルにある列挙を並べる: {out}");

    let (_tmp, shape) = pair(
        "shape",
        PROTO,
        &plain
            .replace("import proto \"order.proto\" MemberTier -> 会員区分", "import proto \"order.proto\" MemberTier"),
    );
    let (code, out) = run(&["check", &shape]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E013") && out.contains("import proto"), "{out}");
}

/// A contract that does not read as a `.proto` is named with where it stops (§15.166), and the
/// rule is not held to what was found before the break. The reader rulec had before read up to
/// what it did not understand and passed over it: a file cut off before its last `}` passed the
/// check with every value it held, and a value written without `=` was skipped, so the check said
/// the contract had lost that value (E032) and sent the reader to the rule.
#[test]
fn 読めない契約は途中まで読まずに場所を言う() {
    let whole = rule("一般(basic) | ゴールド(gold) default", "| 一般 | 800円 |\n| - | 400円 |\n");
    let (_tmp, p) = pair("whole", PROTO, &whole);
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 0, "the rule passes against the whole contract: {out}");

    let cut = PROTO.trim_end().trim_end_matches('}');
    let (_tmp, p) = pair("cut", cut, &whole);
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("エラー[E013]: `order.proto` を読めません"), "{out}");
    assert!(out.contains("`.proto` として読めません（14 行目の 21 文字目）: `}` が要るところにファイルの終わりがあります"), "{out}");

    let stray = PROTO.replace("MEMBER_TIER_GOLD = 2;", "MEMBER_TIER_GOLD 2;");
    let (_tmp, p) = pair("stray", &stray, &whole);
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("エラー[E013]: `order.proto` を読めません") && !out.contains("E032"), "{out}");
    assert!(out.contains("`.proto` として読めません（9 行目の 20 文字目）: `=` が要るところに `2` があります"), "{out}");
}

#[test]
fn 承認者の資料が出どころを書く() {
    let (_tmp, p) = pair("doc", PROTO, &rule("一般(basic) | ゴールド(gold) default", "| 一般 | 800円 |\n| - | 400円 |\n"));
    let (code, out) = run(&["doc", &p]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("order.proto") && out.contains("MemberTier"), "{out}");
    assert!(out.contains("E032"), "承認者は、他所が集合を変えうることを読む必要がある: {out}");
}

/// An enum declared inside a message is named with the message it is in (§15.274). The `.proto`
/// rulec writes for the rule names it `shop.v1.Member.Tier`, which is the name buf resolves, and the
/// service reads it from the class protobuf-py puts it in. It was named as if it were declared at the
/// top of its file (`shop.v1.Tier`): `check` and `gen` passed, and the module did not build.
#[test]
fn 入れ子の列挙はメッセージの名前で引く() {
    let (_t, d) = dir("nested");
    std::fs::write(
        d.join("member.proto"),
        "syntax = \"proto3\";\npackage shop.v1;\nmessage Member {\n  enum Tier {\n    TIER_UNSPECIFIED = 0;\n    TIER_BASIC = 1;\n    TIER_GOLD = 2;\n  }\n}\n",
    )
    .unwrap();
    std::fs::write(
        d.join("fee.rule"),
        "rule fee(fee) v1\n\nenum tier = basic | gold\nimport proto \"member.proto\" Tier -> tier\n\n\
         inputs\n  member : tier\n\noutputs\n  free : bool\n\ntable t\n| member | -> free |\n| basic  | false   |\n| gold   | true    |\n",
    )
    .unwrap();
    let rule = d.join("fee.rule").to_string_lossy().into_owned();
    let out = d.join("out").to_string_lossy().into_owned();
    let (code, said) = run(&rulec::targets::up_to_0_40(&["gen", &rule, "--out", &out]));
    assert_eq!(code, 0, "{said}");
    let body = std::fs::read_to_string(d.join("out/proto/rulec/fee/v1/fee.proto")).unwrap();
    assert!(body.contains("  optional shop.v1.Member.Tier member = 1;\n"), "{body}");
    assert!(body.contains("import \"shop/v1/member.proto\";"), "{body}");
    let svc = std::fs::read_to_string(d.join("out/python/fee_service.py")).unwrap();
    assert!(svc.contains("shop_v1_member_pb.Member.Tier(1): m.Tier.BASIC,"), "{svc}");
    let (code, api) = run(&["api", &rule]);
    assert_eq!(code, 0, "{api}");
    assert!(api.contains("\"type\":\"shop.v1.Member.Tier\""), "{api}");
}

/// A contract whose file name has a character other than a letter, a digit or `_` (§15.274): the
/// plugin writes its stub as `member_tier_pb`, and the service reads that module. The service read
/// `member-tier_pb`, which is not a name Python can import, and no Connect stage could start.
#[test]
fn 契約のファイルの名前の記号はstubの名前で下線になる() {
    let (_t, d) = dir("hyphen");
    std::fs::write(
        d.join("member-tier.proto"),
        "syntax = \"proto3\";\npackage shop.v1;\nenum Tier {\n  TIER_UNSPECIFIED = 0;\n  TIER_BASIC = 1;\n  TIER_GOLD = 2;\n}\n",
    )
    .unwrap();
    std::fs::write(
        d.join("fee.rule"),
        "rule fee(fee) v1\n\nenum tier = basic | gold\nimport proto \"member-tier.proto\" Tier -> tier\n\n\
         inputs\n  member : tier\n\noutputs\n  free : bool\n\ntable t\n| member | -> free |\n| basic  | false   |\n| gold   | true    |\n",
    )
    .unwrap();
    let rule = d.join("fee.rule").to_string_lossy().into_owned();
    let out = d.join("out").to_string_lossy().into_owned();
    let (code, said) = run(&rulec::targets::up_to_0_40(&["gen", &rule, "--out", &out]));
    assert_eq!(code, 0, "{said}");
    assert!(d.join("out/proto/shop/v1/member-tier.proto").exists(), "the contract keeps its name");
    let svc = std::fs::read_to_string(d.join("out/python/fee_service.py")).unwrap();
    assert!(svc.contains("from stubs.shop.v1 import member_tier_pb as shop_v1_member_tier_pb\n"), "{svc}");
    assert!(svc.contains("shop_v1_member_tier_pb.Tier(1): m.Tier.BASIC,"), "{svc}");
    assert!(!svc.contains("member-tier_pb"), "{svc}");
}

/// Runs `rulec` in `dir` with the output in `lang`.
fn run_in(lang: &str, dir: &std::path::Path, args: &[&str]) -> (i32, String) {
    let written_in = if args.contains(&"--lang") { vec![] } else { vec!["--lang", lang] };
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(dir).env("RULEC_LANG", lang).args(args).args(written_in).output().expect("rulec を起動できない");
    let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&o.stderr));
    (o.status.code().unwrap_or(-1), s)
}

/// A rule that reads the enum `source` of `contract.proto` into `kind`, with `values`, and answers
/// for each of them.
fn kind_rule(source: &str, values: &[&str]) -> String {
    let rows: String = values.iter().map(|v| format!("| {v} | false |\n")).collect();
    format!(
        "rule pay(pay) v1\n\nimport proto \"contract.proto\" {source} -> kind\nenum kind = {}\n\ninputs\n  k : kind\n\noutputs\n  due : bool\n\ntable dues\n| k | -> due |\n{rows}",
        values.join(" | ")
    )
}

/// Writes `contract.proto` and `pay.rule` into a new directory, and checks the rule in English and
/// in Japanese.
fn check_pair(tag: &str, proto: &str, rule: &str) -> (TempDir, i32, String, String) {
    let (t, d) = dir(tag);
    std::fs::write(d.join("contract.proto"), proto).unwrap();
    std::fs::write(d.join("pay.rule"), rule).unwrap();
    let (code, en) = run_in("en", &d, &["check", "pay.rule"]);
    let (_, ja) = run_in("ja", &d, &["check", "pay.rule"]);
    (t, code, en, ja)
}

const ALIASED: &str = "syntax = \"proto3\";\npackage claims.v1;\n\nenum Category {\n  option allow_alias = true;\n  CATEGORY_UNSPECIFIED = 0;\n  CATEGORY_TRAVEL = 1;\n  CATEGORY_TRIP = 1;\n  CATEGORY_PURCHASE = 2;\n}\n";

/// Under `allow_alias` the names of one number are one value (§15.59): a rule writes it by any of
/// them, and by one only. The reading counted each name as a value, so a rule had to declare both
/// and treat one value as two.
#[test]
fn the_names_of_one_number_are_one_value() {
    for values in [["travel", "purchase"], ["trip", "purchase"]] {
        let (_t, code, en, _) = check_pair("alias", ALIASED, &kind_rule("Category", &values));
        assert_eq!(code, 0, "{values:?}: {en}");
    }
    let (_t, code, en, ja) = check_pair("alias-twice", ALIASED, &kind_rule("Category", &["travel", "trip", "purchase"]));
    assert_eq!(code, 1, "{en}");
    assert!(en.contains("error[E032]: Enum kind does not agree with Category in contract.proto"), "{en}");
    assert!(en.contains("`travel` and `trip` of this enum are one value in contract.proto, the number 1 (`CATEGORY_TRAVEL`, `CATEGORY_TRIP`). Keep one of them."), "{en}");
    assert!(ja.contains("この列挙の `travel` と `trip` は、contract.proto では番号 1 の一つの値です（`CATEGORY_TRAVEL`、`CATEGORY_TRIP`）。どれか一つにしてください。"), "{ja}");
    let (_t, code, en, ja) = check_pair("alias-none", ALIASED, &kind_rule("Category", &["purchase"]));
    assert_eq!(code, 1, "{en}");
    assert!(en.contains("In contract.proto but not in this enum: travel\n"), "{en}");
    assert!(en.contains("`CATEGORY_TRAVEL` and `CATEGORY_TRIP` in contract.proto are one value, the number 1; a rule may write it as any of `travel`, `trip`."), "{en}");
    assert!(ja.contains("contract.proto の `CATEGORY_TRAVEL` と `CATEGORY_TRIP` は番号 1 の一つの値で、規則には `travel`、`trip` のどれで書いてもかまいません。"), "{ja}");
}

/// The service a rule is generated as sends and reads the value by its number, under the first name
/// the contract gives it, whichever name the rule writes it by.
#[test]
fn the_service_carries_a_value_of_two_names_by_its_number() {
    let (_t, d) = dir("alias-gen");
    std::fs::write(d.join("contract.proto"), ALIASED).unwrap();
    std::fs::write(d.join("pay.rule"), kind_rule("Category", &["trip", "purchase"])).unwrap();
    let (code, said) = run_in("en", &d, &rulec::targets::up_to_0_40(&["gen", "pay.rule", "--out", "out"]));
    assert_eq!(code, 0, "{said}");
    let svc = std::fs::read_to_string(d.join("out/python/pay_service.py")).unwrap();
    assert!(svc.contains("claims_v1_contract_pb.Category(1): m.Kind.TRIP,"), "{svc}");
    let (code, api) = run_in("en", &d, &["api", "pay.rule"]);
    assert_eq!(code, 0, "{api}");
    assert!(api.contains("\"name\":\"trip\",\"alias\":\"CATEGORY_TRAVEL\",\"number\":1"), "{api}");
}

/// A prefix in other cases, or with its `_`s elsewhere, is dropped as buf's form is; a name the
/// enum's name runs on into keeps it. E032 says how the contract's names were read, and a rule
/// written to the whole names is told the names to write (§15.59).
#[test]
fn a_prefix_is_read_whatever_its_case_and_where_a_word_ends() {
    let mixed = "syntax = \"proto3\";\npackage claims.v1;\n\nenum Category {\n  Category_Unspecified = 0;\n  Category_Food = 1;\n  CATEGORYDRINK_SET = 2;\n}\n";
    let (_t, code, en, _) = check_pair("mixed", mixed, &kind_rule("Category", &["food", "categorydrink_set"]));
    assert_eq!(code, 0, "{en}");
    let (_t, code, en, ja) = check_pair("mixed-whole", mixed, &kind_rule("Category", &["category_unspecified", "category_food", "drink_set"]));
    assert_eq!(code, 1, "{en}");
    for said in [
        "`category_unspecified` of this enum is `Category_Unspecified` in contract.proto, the number 0: what proto3 reads as not set, and not a value a rule answers for. Take it out of this enum.",
        "`category_food` of this enum is the whole name of `Category_Food` in contract.proto, which is read as `food`: write `food`.",
        "`Category_Food` in contract.proto is read as `food`, without the enum's name (`Category`) at its front.",
        "In contract.proto but not in this enum: categorydrink_set\n",
        "In this enum but not in contract.proto: drink_set\n",
        "`CATEGORYDRINK_SET` in contract.proto is read whole, as `categorydrink_set`: the `Category` it begins with runs on into a word, with no `_` or capital after it.",
    ] {
        assert!(en.contains(said), "{said}\n{en}");
    }
    for said in [
        "この列挙の `category_unspecified` は contract.proto の `Category_Unspecified` で、番号 0 です。",
        "この列挙の `category_food` は、contract.proto の `Category_Food` の名前をそのまま書いたものです。その値は `food` と読むので、`food` と書いてください。",
        "contract.proto の `Category_Food` は、頭にある列挙の名前（`Category`）を落として `food` と読みます。",
        "contract.proto の `CATEGORYDRINK_SET` は、頭の `Category` のあとが語の切れ目（`_` か大文字）でないので、列挙の名前を落とさずに `categorydrink_set` と読みます。",
    ] {
        assert!(ja.contains(said), "{said}\n{ja}");
    }
    // without its prefix, `COLOR_RED` would be read as `RED` is
    let colors = "syntax = \"proto3\";\npackage claims.v1;\n\nenum Color {\n  COLOR_UNSPECIFIED = 0;\n  COLOR_RED = 1;\n  RED = 2;\n}\n";
    let (_t, code, en, ja) = check_pair("kept", colors, &kind_rule("Color", &["red"]));
    assert_eq!(code, 1, "{en}");
    assert!(en.contains("`COLOR_RED` in contract.proto is read whole, as `color_red`: without the enum's name (`Color`) it would be `red`, which `RED` is read as."), "{en}");
    assert!(ja.contains("contract.proto の `COLOR_RED` は、列挙の名前（`Color`）を落とすと `red` になり、`RED` の読みと同じになるので、落とさずに `color_red` と読みます。"), "{ja}");
}

const TWO_STATUSES: &str = "syntax = \"proto3\";\npackage shop.v1;\n\nmessage Order {\n  enum Status {\n    STATUS_UNSPECIFIED = 0;\n    STATUS_OPEN = 1;\n    STATUS_PAID = 2;\n  }\n  Status status = 1;\n}\n\nmessage Line {\n  enum Status {\n    STATUS_UNSPECIFIED = 0;\n    STATUS_PICKED = 1;\n    STATUS_PACKED = 2;\n  }\n  Status status = 1;\n}\n";

/// Two messages may each nest an enum of one name. The line names one by the messages it is nested
/// in, as a `shape` line names a message; a name that could be either is E013. The first of the two
/// was read, whichever the rule meant.
#[test]
fn an_enum_name_two_messages_nest_is_named_with_its_message() {
    let (_t, code, en, ja) = check_pair("two-status", TWO_STATUSES, &kind_rule("Status", &["open", "paid"]));
    assert_eq!(code, 1, "{en}");
    assert!(en.contains("error[E013]: `contract.proto` has more than one enum named `Status`"), "{en}");
    assert!(en.contains("Say which, by the messages it is nested in: `Order.Status`, `Line.Status`"), "{en}");
    assert!(ja.contains("エラー[E013]: `contract.proto` に、`Status` という名前の列挙が複数あります"), "{ja}");
    for source in ["Order.Status", "shop.v1.Order.Status", "\"Order.Status\""] {
        let (_t, code, en, _) = check_pair("order-status", TWO_STATUSES, &kind_rule(source, &["open", "paid"]));
        assert_eq!(code, 0, "{source}: {en}");
    }
    let (_t, code, en, _) = check_pair("line-status", TWO_STATUSES, &kind_rule("Line.Status", &["picked", "packed"]));
    assert_eq!(code, 0, "{en}");
    // the generated `.proto` names the one the line names
    let (_t, d) = dir("line-gen");
    std::fs::write(d.join("contract.proto"), TWO_STATUSES).unwrap();
    std::fs::write(d.join("pay.rule"), kind_rule("Line.Status", &["picked", "packed"])).unwrap();
    let (code, said) = run_in("en", &d, &rulec::targets::up_to_0_40(&["gen", "pay.rule", "--out", "out"]));
    assert_eq!(code, 0, "{said}");
    let body = std::fs::read_to_string(d.join("out/proto/rulec/pay/v1/pay.proto")).unwrap();
    assert!(body.contains("  optional shop.v1.Line.Status k = 1;\n"), "{body}");
    let svc = std::fs::read_to_string(d.join("out/python/pay_service.py")).unwrap();
    assert!(svc.contains("shop_v1_contract_pb.Line.Status(1): m.Kind.PICKED,"), "{svc}");
}
