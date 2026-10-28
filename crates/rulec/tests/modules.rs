//! Modules (ritsu's DESIGN 17.6): in a module, a path a rule writes that starts with `@<name>/` is
//! read from the root of the dependency `<name>` that the module's `ritsu.mod` requires, the path a
//! dependency's own file writes from that dependency's root, and the single command reads such a
//! path from the command line, with no flag. A path out of the module, into a module nested in it, a
//! dependency the module does not require and a file a dependency does not have are each an error
//! of the rule's own code for a reference that cannot be followed (E044 for `apply`), with the
//! words ritsu-base has for them; outside every module, `@…` is a plain relative path, and the
//! error for a file that is not there says so.
//!
//! The material is `tests/modules/`, in English (`en/`) and in Japanese (`ja/`): the module at hand
//! (`app`), its path dependency (`lib`), the dependency's dependency (`base`, which app reaches only
//! as `@lib/@base/…`), a module nested in app (`app/inner`) and a directory outside every module
//! (`loose`). The commands run in the material's directories, so the paths they print are the ones
//! given to them. A walk of the material's own directory, outside every module, does not enter the
//! modules in it, and says so in a line each. The golden files are `tests/golden/modules/`
//! (`RULEC_BLESS=1` writes them).
//!
//! A contract of the module may import a contract of its dependency by the path of its package
//! (ritsu's DESIGN 17.6): `gen` copies both into the module of the rule's service it writes, and buf
//! builds that module in one run of protoc, which reads one file by one name. Two different files
//! for one path of that module, one of them a dependency's, stop `gen` (exit 2), from one rule or
//! from two; so does a file of a dependency reached by two names, which would be two files of the
//! module that declare the same types. Either stops the run before any rule's files are written.

use ritsu_testkit::{Need, TempDir, ready};
use std::path::Path;
use std::process::Command;

/// `rulec` with `args`, run in `dir` (under `tests/modules/`), with no language from the
/// environment: the command as written, what it printed and how it ended.
fn rulec(dir: &str, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(dir))
        .env_remove("RULEC_LANG")
        .env_remove("RITSU_LANG")
        .args(args)
        .output()
        .expect("cannot start rulec");
    let code = o.status.code().unwrap_or(-1);
    let said = format!("$ rulec {}\n{}{}exit {code}\n\n", args.join(" "), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)).replace(env!("CARGO_PKG_VERSION"), "<version>");
    (code, said)
}

/// The commands in `dir`, each with the exit code it must end with, as one transcript.
fn transcript(out: &mut String, dir: &str, runs: &[(&[&str], i32)]) {
    out.push_str(&format!("# in {dir}\n\n"));
    for (args, want) in runs {
        let (code, said) = rulec(dir, args);
        assert_eq!(code, *want, "{said}");
        out.push_str(&said);
    }
}

#[test]
fn a_module_in_english() {
    let mut t = String::new();
    transcript(
        &mut t,
        "en/app",
        &[
            // the invoice applies lib's order total, which applies base's VAT rate (read as
            // `@lib/@base/…`); the VAT of a category names base's rule through lib itself; the
            // billing is shaped by lib's order.proto
            (&["check", "rules/"], 0),
            (&["check", "rules/", "--format", "json"], 0),
            // the single command reads `@` from the command line
            (&["check", "@lib/rules/order_total.rule"], 0),
            (&["check", "@lib/@base/rules/vat_rate.rule"], 0),
            // the four errors
            (&["check", "errors/not_required.rule"], 1),
            (&["check", "errors/outside.rule"], 1),
            (&["check", "errors/nested.rule"], 1),
            (&["check", "errors/missing.rule"], 1),
            (&["check", "errors/missing.rule", "--format", "json"], 1),
        ],
    );
    // from any directory of the module, `@lib/…` names the same file
    transcript(&mut t, "en/app/rules", &[(&["check", "@lib/rules/order_total.rule"], 0)]);
    // outside every module, `@lib/…` is a path beside the file
    transcript(&mut t, "en/loose", &[(&["check", "invoice.rule"], 1), (&["check", "invoice.rule", "--format", "json"], 1)]);
    // a walk that starts outside every module does not enter one, and says so in a line each
    transcript(&mut t, "en", &[(&["check", "."], 1)]);
    for said in [
        "note: `./app` has a `ritsu.mod` of its own: it is another module, and was not read; run `rulec check ./app` to read it",
        "the `@shop` of `@shop/rules/order_total.rule` is no dependency",
        "`../../lib/rules/order_total.rule` leads outside the module `example.com/acme/app`",
        "`../inner/rules/vat_rate.rule` leads into `inner`, a directory below with a `ritsu.mod` of its own, without an `@`",
        "the dependency `@lib` (`example.com/acme/lib`, `../lib`) has no `rules/order_totals.rule`",
        "a path that starts with `@` names a dependency only in a module",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/en.txt"), &t);
}

#[test]
fn a_module_in_japanese() {
    let mut t = String::new();
    transcript(
        &mut t,
        "ja/アプリ",
        &[
            (&["check", "--lang", "ja", "規則/"], 0),
            (&["check", "--lang", "ja", "規則/", "--format", "json"], 0),
            (&["check", "--lang", "ja", "@lib/規則/注文の合計.rule"], 0),
            (&["check", "--lang", "ja", "@lib/@base/規則/付加価値税率.rule"], 0),
            (&["check", "--lang", "ja", "誤り/依存に無い.rule"], 1),
            (&["check", "--lang", "ja", "誤り/外を指す.rule"], 1),
            (&["check", "--lang", "ja", "誤り/入れ子.rule"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.rule"], 1),
            (&["check", "--lang", "ja", "誤り/無いファイル.rule", "--format", "json"], 1),
        ],
    );
    transcript(&mut t, "ja/アプリ/規則", &[(&["check", "--lang", "ja", "@lib/規則/注文の合計.rule"], 0)]);
    transcript(&mut t, "ja/外", &[(&["check", "--lang", "ja", "請求額.rule"], 1), (&["check", "--lang", "ja", "請求額.rule", "--format", "json"], 1)]);
    transcript(&mut t, "ja", &[(&["check", "--lang", "ja", "."], 1)]);
    for said in [
        "備考: `./アプリ` は自分の `ritsu.mod` を持つ別のモジュールなので、読んでいません。読むには `rulec check ./アプリ` を実行してください",
        "`@shop/規則/注文の合計.rule` の `@shop` は依存ではありません",
        "`../../部品/規則/注文の合計.rule` は、モジュール `example.com/acme/app` の外を指します",
        "`../入れ子/規則/付加価値税率.rule` は、`ritsu.mod` を持つ下のディレクトリ `入れ子` の中を、`@` を通らずに指しています",
        "依存 `@lib`（`example.com/acme/lib`、`../部品`）に `規則/注文合計.rule` はありません",
        "`@` で始まるパスが依存を指すのは、`ritsu.mod` のあるモジュールの中だけです",
    ] {
        assert!(t.contains(said), "{said}\n{t}");
    }
    ritsu_testkit::golden(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/modules/ja.txt"), &t);
}

/// The billing plans of the module (`plan.proto`, which a rule reads its enum from with `import
/// proto`) carry an amount in the money of the dependency `lib`, imported by the path of its package
/// (`acme/orders/v1/money.proto`, under `@lib/proto`). `gen` copies the contract and the dependency's
/// file it imports into the module it writes, byte for byte, each at the path its import names, and
/// at the tools level buf builds that module. Then the module holds a file of its own at that path,
/// with other text, and imports a file of `lib` that imports `lib`'s: each reads its own, and buf's
/// one run of protoc would read one of them for both, so `gen` names the two files and stops (exit
/// 2). In English (`en/`) and in Japanese (`ja/`).
#[test]
fn a_contract_of_the_module_imports_its_dependencys_contract() {
    for (lang, app, lib, rule) in [("en", "app", "lib", "rules/plan_fee.rule"), ("ja", "アプリ", "部品", "規則/プランの料金.rule")] {
        let t = TempDir::new("rulec-modules-proto");
        ritsu_testkit::tmp::copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(lang), t.path());
        let (plan, rule_text) = if lang == "en" {
            (
                "// The plans of billing, and a charge of one in the money of the module lib, imported by\n// the path of its package.\nsyntax = \"proto3\";\n\npackage acme.billing.v1;\n\nimport \"acme/orders/v1/money.proto\";\n\nenum Plan {\n  PLAN_MONTHLY = 0;\n  PLAN_YEARLY = 1;\n}\n\nmessage Charge {\n  Plan plan = 1;\n  acme.orders.v1.Money amount = 2;\n}\n",
                "# The fee of a plan of billing, whose plans are the enum of the module's plan.proto.\nrule plan_fee v1\ndescription \"The fee of a plan of billing, by the plans of the billing service's contract\"\n\nimport proto \"../proto/acme/billing/v1/plan.proto\" Plan -> plan\nenum plan = monthly default | yearly\n\ninputs\n  plan : plan\n\noutputs\n  fee : money[GBPc]  round down(1GBPc)\n\ntable fees\npolicy unique\n| plan    | -> fee : money[GBPc] |\n| monthly | 500GBPc              |\n| yearly  | 5000GBPc             |\n\nexamples\n| plan    | -> fee   |\n| monthly | 500GBPc  |\n| yearly  | 5000GBPc |\n",
            )
        } else {
            (
                "// 請求のプランと、その一回の請求。金額はモジュール lib の金額で、パッケージのパスで import する。\nsyntax = \"proto3\";\n\npackage acme.billing.v1;\n\nimport \"acme/orders/v1/money.proto\";\n\nenum Plan {\n  PLAN_MONTHLY = 0;\n  PLAN_YEARLY = 1;\n}\n\nmessage Charge {\n  Plan plan = 1;\n  acme.orders.v1.Money amount = 2;\n}\n",
                "# 請求のプランの料金。プランは、このモジュールの plan.proto の列挙である。\nrule プランの料金(plan_fee) v1\ndescription \"請求のプランごとの料金。プランは請求のサービスの契約のもの\"\n\nimport proto \"../proto/acme/billing/v1/plan.proto\" Plan -> プラン\nenum プラン(plan) = 月払い(monthly) default | 年払い(yearly)\n\ninputs\n  プラン(plan) : プラン\n\noutputs\n  料金(fee) : money[GBPc]  round down(1GBPc)\n\ntable 料金表(fees)\npolicy unique\n| プラン | -> 料金(fee) : money[GBPc] |\n| 月払い | 500GBPc                    |\n| 年払い | 5000GBPc                   |\n\nexamples\n| プラン | -> 料金   |\n| 月払い | 500GBPc  |\n| 年払い | 5000GBPc |\n",
            )
        };
        t.write(&format!("{app}/proto/acme/billing/v1/plan.proto"), plan);
        t.write(&format!("{app}/{rule}"), rule_text);
        let out = t.path().join("out");
        let generate = || {
            let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
                .current_dir(t.path().join(app))
                .env_remove("RULEC_LANG")
                .env_remove("RITSU_LANG")
                .args(["gen", rule, "--out", out.to_str().unwrap(), "--lang", lang])
                .output()
                .expect("cannot start rulec");
            (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
        };
        let (code, stdout, stderr) = generate();
        assert_eq!(code, 0, "{stdout}{stderr}");
        let money = format!("{lib}/proto/acme/orders/v1/money.proto");
        assert_eq!(std::fs::read(out.join("proto/acme/orders/v1/money.proto")).unwrap(), std::fs::read(t.path().join(&money)).unwrap(), "{lang}: the dependency's file, as it is");
        assert_eq!(std::fs::read_to_string(out.join("proto/acme/billing/v1/plan.proto")).unwrap(), plan);
        if ready(Need::Buf, || Command::new("buf").arg("--version").output().is_ok_and(|o| o.status.success()), "buf is not installed; the module of the rule's service is not built") {
            let o = Command::new("buf").arg("build").current_dir(out.join("proto")).output().unwrap();
            assert!(o.status.success(), "{lang}: buf build refuses the module:\n{}", String::from_utf8_lossy(&o.stderr));
        }
        // a file of the module at the path of lib's money, which a file of lib's reaches by its own
        t.write(&format!("{lib}/proto/acme/orders/v1/total.proto"), "// The total of an order of the module lib, in its own money.\nsyntax = \"proto3\";\n\npackage acme.orders.v1;\n\nimport \"acme/orders/v1/money.proto\";\n\nmessage Total {\n  Money amount = 1;\n}\n");
        t.write(&format!("{app}/proto/acme/orders/v1/money.proto"), "// The module's own money, at the path of the money of the module lib.\nsyntax = \"proto3\";\n\npackage acme.orders.v1;\n\nmessage Money {\n  int64 units = 1;\n  string currency = 2;\n}\n");
        t.write(
            &format!("{app}/proto/acme/billing/v1/plan.proto"),
            plan.replace("import \"acme/orders/v1/money.proto\";\n", "import \"acme/orders/v1/money.proto\";\nimport \"acme/orders/v1/total.proto\";\n").replace("  acme.orders.v1.Money amount = 2;\n", "  acme.orders.v1.Money amount = 2;\n  acme.orders.v1.Total total = 3;\n"),
        );
        let (code, stdout, stderr) = generate();
        assert_eq!((code, stdout.as_str()), (2, ""), "{stderr}");
        let want = if lang == "en" {
            "error: `@lib/proto/acme/orders/v1/money.proto` and `proto/acme/orders/v1/money.proto` are two different files, and both would be `proto/acme/orders/v1/money.proto` of the module generated, imported as `acme/orders/v1/money.proto`; protoc reads one name as one file, so nothing is generated\n"
        } else {
            "error: `@lib/proto/acme/orders/v1/money.proto` と `proto/acme/orders/v1/money.proto` は中身の違うファイルですが、どちらも生成するモジュールの `proto/acme/orders/v1/money.proto` になり、`acme/orders/v1/money.proto` として import されます。protoc は一つの名前を一つのファイルとして読むので、生成しません\n"
        };
        assert_eq!(stderr, want);
    }
}

/// `rulec gen` of `rules` in the module `app` of a copy of the English material, in `lang`, into the
/// directory `out` of the copy: how it ended, what it printed and what it said on stderr.
fn gen_in(t: &TempDir, rules: &[&str], lang: &str, out: &str) -> (i32, String, String) {
    let out = t.path().join(out);
    let mut args = vec!["gen"];
    args.extend_from_slice(rules);
    args.extend(["--out", out.to_str().unwrap(), "--lang", lang]);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(t.path().join("app")).env_remove("RULEC_LANG").env_remove("RITSU_LANG").args(&args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The plans of billing, whose charge is in lib's money, imported by the path of its package.
const PLAN: &str = "// The plans of billing, and a charge of one in the money of the module lib, imported by\n// the path of its package.\nsyntax = \"proto3\";\n\npackage acme.billing.v1;\n\nimport \"acme/orders/v1/money.proto\";\n\nenum Plan {\n  PLAN_MONTHLY = 0;\n  PLAN_YEARLY = 1;\n}\n\nmessage Charge {\n  Plan plan = 1;\n  acme.orders.v1.Money amount = 2;\n}\n";

/// The regions of billing, whose fee is in lib's money, imported from lib's root as lib's own files
/// import it (`proto/acme/…`).
const REGION: &str = "// The regions of billing, and a fee in the money of the module lib, imported from the root of\n// lib as lib's order imports it.\nsyntax = \"proto3\";\n\npackage acme.billing.v1;\n\nimport \"proto/acme/orders/v1/money.proto\";\n\nenum Region {\n  REGION_HOME = 0;\n  REGION_ABROAD = 1;\n}\n\nmessage Fee {\n  Region region = 1;\n  acme.orders.v1.Money amount = 2;\n}\n";

/// A rule of the fee of a plan of billing (`plan`) or of a region (`region`), whose values are the
/// enum of that contract.
fn fee_rule(name: &str, enum_name: &str, contract: &str, values: [&str; 2]) -> String {
    let [a, b] = values;
    format!(
        "# The fee of a {enum_name} of billing, by the enum of the billing service's contract.\nrule {name} v1\ndescription \"The fee of a {enum_name} of billing\"\n\nimport proto \"../proto/acme/billing/v1/{contract}\" {} -> {enum_name}\nenum {enum_name} = {a} default | {b}\n\ninputs\n  {enum_name} : {enum_name}\n\noutputs\n  fee : money[GBPc]  round down(1GBPc)\n\ntable fees\npolicy unique\n| {enum_name} | -> fee : money[GBPc] |\n| {a} | 500GBPc |\n| {b} | 5000GBPc |\n",
        enum_name[..1].to_uppercase() + &enum_name[1..]
    )
}

/// One file of lib, its money, reached by two names: by the path of its package from the plans
/// (`acme/orders/v1/money.proto`, under `@lib/proto`) and from lib's root by the regions
/// (`proto/acme/orders/v1/money.proto`, under `@lib`). Each is a file the check reads; `gen` would
/// copy it to two paths of the module of the rule's service, which protoc reads as two files that
/// declare the same types, so it names the file and the two names and stops (exit 2), from one rule
/// that reads both contracts and from two rules that read one each, before it writes any rule's
/// files. In English and in Japanese.
#[test]
fn a_file_of_a_dependency_reached_by_two_names_stops_gen() {
    let t = TempDir::new("rulec-modules-two-names");
    ritsu_testkit::tmp::copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules/en"), t.path());
    t.write("app/proto/acme/billing/v1/plan.proto", PLAN);
    t.write("app/proto/acme/billing/v1/region.proto", REGION);
    t.write("app/rules/plan_fee.rule", fee_rule("plan_fee", "plan", "plan.proto", ["monthly", "yearly"]));
    t.write("app/rules/region_fee.rule", fee_rule("region_fee", "region", "region.proto", ["home", "abroad"]));
    t.write(
        "app/rules/billing_fee.rule",
        "# The fee of a plan of billing in a region, by the enums of the billing service's two contracts.\nrule billing_fee v1\ndescription \"The fee of a plan of billing in a region\"\n\nimport proto \"../proto/acme/billing/v1/plan.proto\" Plan -> plan\nenum plan = monthly default | yearly\nimport proto \"../proto/acme/billing/v1/region.proto\" Region -> region\nenum region = home default | abroad\n\ninputs\n  plan   : plan\n  region : region\n\noutputs\n  fee : money[GBPc]  round down(1GBPc)\n\ntable fees\npolicy unique\n| plan    | region | -> fee : money[GBPc] |\n| monthly | home   | 500GBPc              |\n| monthly | abroad | 700GBPc              |\n| yearly  | home   | 5000GBPc             |\n| yearly  | abroad | 7000GBPc             |\n",
    );
    // each rule passes check, and each of the first two is generated by itself
    for rule in ["rules/plan_fee.rule", "rules/region_fee.rule", "rules/billing_fee.rule"] {
        let (code, said) = rulec_at(t.path(), "app", &["check", rule]);
        assert_eq!(code, 0, "{said}");
    }
    for rule in ["rules/plan_fee.rule", "rules/region_fee.rule"] {
        let (code, out, err) = gen_in(&t, &[rule], "en", "out");
        assert_eq!(code, 0, "{rule}: {out}{err}");
    }
    let en = "error: `@lib/proto/acme/orders/v1/money.proto` is imported by two names, `acme/orders/v1/money.proto` and `proto/acme/orders/v1/money.proto`, and would be both `proto/acme/orders/v1/money.proto` and `proto/proto/acme/orders/v1/money.proto` of the module generated; protoc reads the two as two files that declare the same types, so nothing is generated\n";
    let ja = "error: `@lib/proto/acme/orders/v1/money.proto` は、`acme/orders/v1/money.proto` と `proto/acme/orders/v1/money.proto` の二つの名前で import され、生成するモジュールの `proto/acme/orders/v1/money.proto` と `proto/proto/acme/orders/v1/money.proto` の二つになります。protoc は二つを別のファイルとして読み、同じ型を二度宣言するので、生成しません\n";
    for (lang, want) in [("en", en), ("ja", ja)] {
        let (code, out, err) = gen_in(&t, &["rules/billing_fee.rule"], lang, &format!("one-{lang}"));
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", want), "one rule ({lang})");
        // the regions' stop the run before anything is written: the plans' files, which come first,
        // are not written either
        let (code, out, err) = gen_in(&t, &["rules/plan_fee.rule", "rules/region_fee.rule"], lang, &format!("two-{lang}"));
        assert_eq!((code, out.as_str(), err.as_str()), (2, "", want), "two rules ({lang})");
        assert!(!t.path().join(format!("two-{lang}")).exists(), "two rules ({lang}): nothing is written");
    }
}

/// Two rules, each with a contract of its own at one path of the module of the rule's service: lib's
/// money, which lib's statuses import, and app's own money with its currencies, at the same path in
/// app. Each rule passes check and is generated by itself; generated together, the module would hold
/// one of the two for both, and one of them is a dependency's, so `gen` names the two files and stops
/// (exit 2), as it does for one rule's two contracts, before it writes any rule's files: the first
/// rule's are not left behind.
#[test]
fn two_rules_with_two_contracts_at_one_path_one_a_dependencys_stop_gen() {
    let t = TempDir::new("rulec-modules-two-rules");
    ritsu_testkit::tmp::copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules/en"), t.path());
    t.write(
        "lib/proto/acme/orders/v1/status.proto",
        "// The statuses of an order of the module lib, and a payment in its own money.\nsyntax = \"proto3\";\n\npackage acme.orders.v1;\n\nimport \"acme/orders/v1/money.proto\";\n\nenum Status {\n  STATUS_OPEN = 0;\n  STATUS_PAID = 1;\n}\n\nmessage Payment {\n  Status status = 1;\n  Money amount = 2;\n}\n",
    );
    t.write(
        "app/proto/acme/orders/v1/money.proto",
        "// The module's own money, at the path of the money of the module lib, with its currencies.\nsyntax = \"proto3\";\n\npackage acme.orders.v1;\n\nenum Currency {\n  CURRENCY_GBP = 0;\n  CURRENCY_EUR = 1;\n}\n\nmessage Money {\n  int64 units = 1;\n  Currency currency = 2;\n}\n",
    );
    t.write(
        "app/rules/status_fee.rule",
        "# The fee of an order by its status, by the enum of lib's contract.\nrule status_fee v1\ndescription \"The fee of an order by its status\"\n\nimport proto \"@lib/proto/acme/orders/v1/status.proto\" Status -> status\nenum status = open default | paid\n\ninputs\n  status : status\n\noutputs\n  fee : money[GBPc]  round down(1GBPc)\n\ntable fees\npolicy unique\n| status | -> fee : money[GBPc] |\n| open   | 0GBPc                |\n| paid   | 100GBPc              |\n",
    );
    t.write(
        "app/rules/currency_fee.rule",
        "# The fee of a payment by its currency, by the enum of the module's own money.\nrule currency_fee v1\ndescription \"The fee of a payment by its currency\"\n\nimport proto \"../proto/acme/orders/v1/money.proto\" Currency -> currency\nenum currency = gbp default | eur\n\ninputs\n  currency : currency\n\noutputs\n  fee : money[GBPc]  round down(1GBPc)\n\ntable fees\npolicy unique\n| currency | -> fee : money[GBPc] |\n| gbp      | 0GBPc                |\n| eur      | 50GBPc               |\n",
    );
    for rule in ["rules/status_fee.rule", "rules/currency_fee.rule"] {
        let (code, said) = rulec_at(t.path(), "app", &["check", rule]);
        assert_eq!(code, 0, "{said}");
        let (code, out, err) = gen_in(&t, &[rule], "en", "out");
        assert_eq!(code, 0, "{rule}: {out}{err}");
    }
    let (code, out, err) = gen_in(&t, &["rules/status_fee.rule", "rules/currency_fee.rule"], "en", "both");
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (
            2,
            "",
            "error: `@lib/proto/acme/orders/v1/money.proto` and `proto/acme/orders/v1/money.proto` are two different files, and both would be `proto/acme/orders/v1/money.proto` of the module generated, imported as `acme/orders/v1/money.proto`; protoc reads one name as one file, so nothing is generated\n"
        )
    );
    assert!(!t.path().join("both").exists(), "nothing is written: the first rule's files neither");
}

/// A path given on the command line that starts with `@` is read as a path written in a file is
/// (ritsu's DESIGN 17.6), from the module the command runs in: a dependency the module does not
/// require, a path out of the module, a module nested in a dependency entered without an `@` and a
/// file a dependency does not have are said in the same words (exit 2), in English and in Japanese,
/// rather than that the path cannot be read. Outside every module, `@…` is a plain relative path, and
/// what rulec says of it is what it said before.
#[test]
fn the_four_errors_of_a_path_given_on_the_command_line() {
    for (lang, app, lib, rule) in [("en", "app", "lib", "rules/order_total.rule"), ("ja", "アプリ", "部品", "規則/注文の合計.rule")] {
        let t = TempDir::new("rulec-modules-given");
        ritsu_testkit::tmp::copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/modules").join(lang), t.path());
        t.write(&format!("{lib}/nested/ritsu.mod"), "module example.com/acme/nested\nversion 0.1.0\n");
        let said = |path: &str| {
            let mut args = vec!["check", path];
            if lang == "ja" {
                args.extend(["--lang", "ja"]);
            }
            rulec_at(t.path(), app, &args)
        };
        let errors = [
            (format!("@shop/{rule}"), [
                format!("error: the `@shop` of `@shop/{rule}` is no dependency: the `ritsu.mod` of the module `example.com/acme/app` has no `require shop …`\n"),
                format!("error: `@shop/{rule}` の `@shop` は依存ではありません。モジュール `example.com/acme/app` の `ritsu.mod` に `require shop …` がありません\n"),
            ]),
            (format!("@lib/../../{lib}/{rule}"), [
                format!("error: `@lib/../../{lib}/{rule}` leads outside the module `example.com/acme/app`; a file in a module names files under the module's root, and in a dependency by `@<name>/`\n"),
                format!("error: `@lib/../../{lib}/{rule}` は、モジュール `example.com/acme/app` の外を指します。モジュールの中のファイルが指せるのは、モジュールのルートの下と、`@<名前>/` で入る依存の中です\n"),
            ]),
            ("@lib/nested/fee.rule".to_string(), [
                "error: `@lib/nested/fee.rule` leads into `@lib/nested`, a directory below with a `ritsu.mod` of its own, without an `@`; require that module, and name its files by `@<name>/`\n".to_string(),
                "error: `@lib/nested/fee.rule` は、`ritsu.mod` を持つ下のディレクトリ `@lib/nested` の中を、`@` を通らずに指しています。そのモジュールを `require` に足し、`@<名前>/` で指してください\n".to_string(),
            ]),
            ("@lib/fee.rule".to_string(), [
                format!("error: the dependency `@lib` (`example.com/acme/lib`, `../{lib}`) has no `fee.rule`\n"),
                format!("error: 依存 `@lib`（`example.com/acme/lib`、`../{lib}`）に `fee.rule` はありません\n"),
            ]),
        ];
        for (path, [en, ja]) in &errors {
            assert_eq!(said(path), (2, if lang == "ja" { ja.clone() } else { en.clone() }), "{lang} {path}");
        }
        // the file that is there
        assert_eq!(said(&format!("@lib/{rule}")).0, 0);
        // outside every module, as before
        assert_eq!(rulec_at(t.path(), ".", &["check", "@lib/fee.rule"]), (2, "error: cannot read `@lib/fee.rule`\n".to_string()));
    }
}

/// `rulec` with `args`, run in `dir` under `root`: how it ended, and what it said.
fn rulec_at(root: &Path, dir: &str, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root.join(dir)).env_remove("RULEC_LANG").env_remove("RITSU_LANG").args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}
