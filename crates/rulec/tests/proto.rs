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
