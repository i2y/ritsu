//! `ritsu gen` of a module (DESIGN 17.8; PLAN B.8): the example `shop`, and its twin with Japanese
//! names (`店`), as packages that hold the files of the dependencies its files reach, each under
//! `deps/<name>/` of the package, a level for each name of the chain (`deps/pricing/deps/tax/`);
//! the heads that name the module a part is made from; `--check` and the JSON's `from`; a file of a
//! dependency that nothing reaches, which is left out; two modules with a file of one name; a name
//! that must be one at run time and that two modules make (a Temporal workflow type, a name of
//! Cedar's), which stops the run; a contribution of the module to a dependency's rule, which check
//! and gen answer alike; the tree's errors and what stops it, said as `ritsu check` says them; the
//! note of `--check` on the ritsu that wrote the lock; a dependency named with a word Python keeps;
//! and a `.proto` of the module that imports a dependency's by the path of its package (DESIGN
//! 17.6). At the tools level, the packages pass `tsc --strict`, `mypy --strict` and `go vet` (the
//! type checkers of tests/gen.rs, in tests/type_checkers), and protoc builds the `.proto` from the
//! roots ritsu reads it from, in their order.
//!
//! The material is tests/projects/modules (`shop`, `pricing`, `tax`) and tests/projects/modules.ja,
//! copied where a test adds to it. The golden files are tests/golden/modules/gen-*.txt and
//! gen-*.json (`RITSU_BLESS=1` writes them).

mod type_checkers;

use ritsu_base::modules::lock::{Lock, Locked};
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::{self, Store};
use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;
use type_checkers::*;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The module the Go is vetted in, and the import path of the package's directory in it.
const MODULE: &str = "gocheck/generated";

/// `ritsu`, run in `dir` with the store at `store` and no language asked of the environment: the
/// exit code, what it printed and what it said on stderr.
fn ritsu_in(dir: &Path, store: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args).env(store::VARIABLE, store);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `ritsu gen` of the module at `dir`, run there, writing under `out`, its Go in the module
/// [`MODULE`].
fn generate(dir: &Path, store: &Path, out: &Path, more: &[&str]) -> (i32, String, String) {
    let o = out.to_str().unwrap();
    let mut args = vec!["gen", "--out", o, "--module", MODULE];
    args.extend_from_slice(more);
    ritsu_in(dir, store, &args)
}

/// The modules of the example (`shop`, `pricing`, `tax`), copied into a directory of their own.
fn example() -> TempDir {
    let t = TempDir::new("modules-gen");
    for m in ["shop", "pricing", "tax"] {
        ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/modules").join(m), &t.path().join(m));
    }
    t
}

/// The file `rel` of `t`, with `from` replaced by `to` (once, and it must be there).
fn replace(t: &TempDir, rel: &str, from: &str, to: &str) {
    let text = t.read(rel);
    assert!(text.contains(from), "{rel} has no `{from}`");
    t.write(rel, text.replacen(from, to, 1));
}

/// Every file under `dir`, from it, in order.
fn files(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

fn assert_has(file: &Path, what: &str) {
    let text = read(file);
    assert!(text.contains(what), "{} has no\n{what}\nin:\n{text}", file.display());
}

fn golden(path: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join(path), got) {
        failures.push(e);
    }
}

/// The shape of the packages under `out` of the module at `project`: each file, what wrote it (its
/// head, DESIGN 9.2), and for a part made from a file of the project, that file as the head names it
/// (the canonical way, `@pricing/…` for a dependency's) with the module the head names, the file read
/// where the module's tree says it is to hold the digest the head writes.
fn shape(out: &Path, project: &Path) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut shape = String::new();
    for target in ["typescript", "python", "go"] {
        shape.push_str(&format!("{target}:\n"));
        for f in files(&out.join(target)) {
            let text = read(&out.join(target).join(&f));
            let mut lines = text.lines();
            let (first, second) = (lines.next().unwrap_or(""), lines.next().unwrap_or(""));
            let by = if f.ends_with("py.typed") {
                "(empty)".to_string()
            } else if f.ends_with("package.json") || f.ends_with("pyproject.toml") {
                "ritsu".to_string()
            } else {
                let tool = ["rulec", "koyomi", "chobo", "dandori", "kaname", "sekisho", "ritsu"].into_iter().find(|t| first.ends_with(&format!("Code generated by {t} {version}. DO NOT EDIT."))).unwrap_or_else(|| panic!("{target}/{f} begins otherwise: {first}"));
                tool.to_string()
            };
            let from = match second.split_once("Source: ") {
                Some((_, s)) => {
                    let path = s.split(" (").next().unwrap();
                    let digest: String = s.split("sha256:").nth(1).unwrap_or_else(|| panic!("{target}/{f}: {s}")).chars().take(16).collect();
                    let module = s.split(", module ").nth(1).map(|m| format!(" [{}]", m.trim_end_matches(')'))).unwrap_or_default();
                    let at = project.join(path);
                    let disk = ritsu_base::modules::map(&at).unwrap_or(at);
                    let bytes = std::fs::read(&disk).unwrap_or_else(|_| panic!("{target}/{f} names {path}, which is not a file of the module's tree"));
                    assert_eq!(digest, ritsu_base::sha256::hex(&bytes)[..16], "{target}/{f}: the digest of {path}");
                    format!(" <- {path}{module}")
                }
                None => String::new(),
            };
            shape.push_str(&format!("  {f}: {by}{from}\n"));
        }
    }
    shape.replace(version, "<version>")
}

// ── the example ──────────────────────────────────────────────────────────────────────────────────

/// The example's `shop`, and `店`: each a package in the three languages, with the module's files
/// at the package's directory as before there were modules, and the files of `pricing` its files
/// reach under `deps/pricing/`, with the file of `tax` that `pricing`'s order total applies under
/// `deps/pricing/deps/tax/`. Each part made from a dependency's file names it the canonical way in
/// its head, with the module and its version. The golden files are the shape of the packages, and
/// what `ritsu gen` printed in English, in Japanese and as JSON (the directory written is `<out>`);
/// the package just written is not stale.
#[test]
fn the_example_module_is_packaged_with_the_dependencies_it_reaches() {
    let empty = TempDir::new("modules-gen-no-store");
    let mut failures = Vec::new();
    for (dir, name) in [("tests/projects/modules/shop", "shop"), ("tests/projects/modules.ja/店", "店")] {
        let project = here().join(dir);
        let t = TempDir::new("modules-gen-example");
        let out = t.path().join("out");
        let (code, stdout, err) = generate(&project, empty.path(), &out, &[]);
        assert_eq!(code, 0, "{name}: {stdout}{err}");
        golden(&format!("tests/golden/modules/gen-{name}.txt"), &shape(&out, &project), &mut failures);
        golden(&format!("tests/golden/modules/gen-{name}.en.txt"), &stdout.replace(&out.display().to_string(), "<out>"), &mut failures);
        for (more, golden_name) in [(&["--lang", "ja"][..], format!("gen-{name}.ja.txt")), (&["--format", "json"][..], format!("gen-{name}.json"))] {
            let other = t.path().join(&golden_name);
            let (code, stdout, err) = generate(&project, empty.path(), &other, more);
            assert_eq!(code, 0, "{name} {more:?}: {stdout}{err}");
            golden(&format!("tests/golden/modules/{golden_name}"), &stdout.replace(&other.display().to_string(), "<out>"), &mut failures);
        }
        let (code, stdout, _) = generate(&project, empty.path(), &out, &["--check"]);
        assert_eq!((code, stdout.as_str()), (0, ""), "{name}: a package just written is not stale");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // what the module's parts import of the dependency's, and the dependency's of its own
    let t = TempDir::new("modules-gen-imports");
    let out = t.path().join("out");
    let (code, _, err) = generate(&here().join("tests/projects/modules/shop"), empty.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
    assert_has(&out.join("typescript/flows/place_order/rules.ts"), "from \"../../deps/pricing/rules/order_total\";\n");
    assert_has(&out.join("typescript/glue/basket.ts"), "import * as rule_pricing_order_total from \"../deps/pricing/rules/order_total\";\n");
    assert_has(&out.join("typescript/deps/pricing/rules/order_total.ts"), "from \"../../../units\";\n");
    assert_has(&out.join("typescript/index.ts"), "export * as deps from \"./deps/index\";\n");
    assert_has(&out.join("typescript/deps/index.ts"), "export * as pricing from \"./pricing/index\";\n");
    assert_has(&out.join("typescript/deps/pricing/index.ts"), "export * as deps from \"./deps/index\";\nexport * as rules from \"./rules/index\";\n");
    assert_has(&out.join("python/generated/flows/place_order/rules.py"), "from ...deps.pricing.rules.order_total import ");
    assert_has(&out.join("python/generated/glue/basket.py"), "from ..deps.pricing.rules import order_total as _rule_pricing_order_total\n");
    assert_has(&out.join("python/generated/deps/pricing/rules/order_total.py"), "from ....units import GBPc as GBPc\n");
    assert_has(&out.join("go/flows/place_order/rules.go"), &format!("\"{MODULE}/deps/pricing/rules/ordertotal\"\n"));
    assert_has(&out.join("go/glue/basket/basket.go"), &format!("pricingordertotal \"{MODULE}/deps/pricing/rules/ordertotal\"\n"));
    assert_has(&out.join("go/deps/pricing/rules/ordertotal/order_total.go"), &format!("\"{MODULE}/units\"\n"));
    let head = "Source: @pricing/@tax/rules/vat_rate.rule (rule vat_rate v1, sha256:";
    for f in ["typescript/deps/pricing/deps/tax/rules/vat_rate.ts", "python/generated/deps/pricing/deps/tax/rules/vat_rate.py", "go/deps/pricing/deps/tax/rules/vatrate/vat_rate.go"] {
        assert_has(&out.join(f), head);
        assert_has(&out.join(f), ", module example.com/acme/tax 1.0.0)\n");
    }
    // in Japanese, the head is the Japanese one
    let ja = t.path().join("ja");
    let (code, _, err) = generate(&here().join("tests/projects/modules/shop"), empty.path(), &ja, &["--lang", "ja", "--target", "typescript"]);
    assert_eq!(code, 0, "{err}");
    assert_has(&ja.join("typescript/deps/pricing/deps/tax/rules/vat_rate.ts"), "、モジュール example.com/acme/tax 1.0.0）\n");
}

/// `--check` compares the dependencies' parts too, and `--format json` names the file each part is
/// made from the canonical way. A file of a dependency that no file reaches is not in the package;
/// once the module's flow reads it, it is, and the package that was written is missing it.
#[test]
fn gen_check_and_the_json_hold_the_dependencies_parts() {
    let t = example();
    let shop = t.path().join("shop");
    let out = t.path().join("out");
    let o = out.display().to_string();
    let (code, _, err) = generate(&shop, t.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
    // the JSON's `from`
    let (code, stdout, stderr) = generate(&shop, t.path(), &out, &["--check", "--format", "json"]);
    assert_eq!((code, stderr.as_str()), (0, ""));
    let v = json(&stdout);
    let from = |rel: &str| -> serde_json::Value {
        let f = v["files"].as_array().unwrap().iter().find(|f| f["path"] == format!("{o}/{rel}")).unwrap_or_else(|| panic!("no {rel}"));
        assert_eq!(f["state"], "same", "{rel}");
        f["from"].clone()
    };
    assert_eq!(from("typescript/deps/pricing/rules/order_total.ts"), "@pricing/rules/order_total.rule");
    assert_eq!(from("python/generated/deps/pricing/deps/tax/rules/vat_rate.py"), "@pricing/@tax/rules/vat_rate.rule");
    assert_eq!(from("go/flows/place_order/workflow.go"), "orders/place_order.flow");
    assert_eq!(from("typescript/deps/pricing/index.ts"), serde_json::Value::Null);
    // a dependency's file changed: its parts in every language, and nothing else
    replace(&t, "tax/rules/vat_rate.rule", "The rates are the example's own\"", "The rates are the example's own, again\"");
    let (code, stdout, _) = generate(&shop, t.path(), &out, &["--check"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!(
            "generated file is stale or hand-edited: {o}/typescript/deps/pricing/deps/tax/rules/vat_rate.ts\ngenerated file is stale or hand-edited: {o}/python/generated/deps/pricing/deps/tax/rules/vat_rate.py\ngenerated file is stale or hand-edited: {o}/go/deps/pricing/deps/tax/rules/vatrate/vat_rate.go\n"
        )
    );
    let (code, _, _) = generate(&shop, t.path(), &out, &[]);
    assert_eq!(code, 0);
    // a file of pricing that nothing reaches
    let fee = t.read("pricing/rules/shipping_fee.rule");
    t.write("pricing/rules/gift_wrap.rule", fee.replacen("rule shipping_fee v1", "rule gift_wrap v1", 1));
    let before = files(&out);
    let (code, stdout, _) = generate(&shop, t.path(), &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""), "a file no file reaches is not in the package");
    assert!(!before.iter().any(|f| f.contains("gift_wrap")), "{before:?}");
    // the flow reads it now
    replace(&t, "shop/orders/place_order.flow", "use rule total from \"@pricing/rules/order_total.rule\"\n", "use rule total from \"@pricing/rules/order_total.rule\"\nuse rule wrap from \"@pricing/rules/gift_wrap.rule\"\n");
    let (code, stdout, _) = generate(&shop, t.path(), &out, &["--check"]);
    assert_eq!(code, 1, "{stdout}");
    for f in ["typescript/deps/pricing/rules/gift_wrap.ts", "python/generated/deps/pricing/rules/gift_wrap.py", "go/deps/pricing/rules/giftwrap/gift_wrap.go"] {
        assert!(stdout.contains(&format!("missing: {o}/{f}\n")), "{f}: {stdout}");
    }
    let (code, _, _) = generate(&shop, t.path(), &out, &["--check", "--lang", "ja"]);
    assert_eq!(code, 1);
}

/// The module requires `tax` as `pricing` does, so `tax` is one module of the tree, `@tax`, and its
/// parts are in the package once, under `deps/tax/` (the canonical way, DESIGN 17.7). `pricing` has
/// a file of the name `tax`'s has, `rules/vat_rate.rule`, of a rule of the same name: a function of
/// the module reads both, and the two are two parts, imported by two names.
#[test]
fn two_modules_with_a_file_of_one_name_are_two_parts() {
    let t = example();
    replace(&t, "shop/ritsu.mod", "require pricing path \"../pricing\"\n", "require pricing path \"../pricing\"\nrequire tax path \"../tax\"\n");
    t.write("pricing/rules/vat_rate.rule", t.read("tax/rules/vat_rate.rule").replace("The rates are the example's own\"", "The rates pricing keeps by itself, the example's own\""));
    t.write("shop/checkout/vat.kn", VAT_KN);
    let shop = t.path().join("shop");
    let (code, stdout, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{stdout}{err}");
    let out = t.path().join("out");
    let (code, _, err) = generate(&shop, t.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let all = files(&out);
    let vat: Vec<&String> = all.iter().filter(|f| f.contains("vat_rate.") || f.contains("vatrate/")).collect();
    assert_eq!(
        vat,
        [
            "go/deps/pricing/rules/vatrate/vat_rate.go",
            "go/deps/tax/rules/vatrate/vat_rate.go",
            "python/generated/deps/pricing/rules/vat_rate.py",
            "python/generated/deps/tax/rules/vat_rate.py",
            "typescript/deps/pricing/rules/vat_rate.ts",
            "typescript/deps/tax/rules/vat_rate.ts"
        ]
    );
    assert!(!all.iter().any(|f| f.contains("deps/pricing/deps/")), "tax is one module of the tree: {all:?}");
    assert_has(&out.join("typescript/deps/pricing/rules/order_total.ts"), "// Applies: vat = @tax/rules/vat_rate.rule (meaning:");
    assert_has(&out.join("typescript/glue/vat.ts"), "import * as rule_pricing_vat_rate from \"../deps/pricing/rules/vat_rate\";\nimport * as rule_tax_vat_rate from \"../deps/tax/rules/vat_rate\";\n");
    assert_has(&out.join("python/generated/glue/vat.py"), "from ..deps.pricing.rules import vat_rate as _rule_pricing_vat_rate\nfrom ..deps.tax.rules import vat_rate as _rule_tax_vat_rate\n");
    assert_has(&out.join("go/glue/vat/vat.go"), &format!("\tpricingvatrate \"{MODULE}/deps/pricing/rules/vatrate\"\n\ttaxvatrate \"{MODULE}/deps/tax/rules/vatrate\"\n"));
    assert_has(&out.join("typescript/deps/index.ts"), "export * as pricing from \"./pricing/index\";\nexport * as tax from \"./tax/index\";\n");
}

/// A function of the module that reads `tax`'s rate and `pricing`'s rate of the same file name.
const VAT_KN: &str = "# The VAT on a basket: at the rate of the module tax, and at the rate the module pricing keeps in a\n# file of the same name.\nmodule vat v1\ndescription \"The VAT on a basket's subtotal, at the rate of the module tax and at the rate the module pricing keeps\"\n\nuse rule tax_rate from \"@tax/rules/vat_rate.rule\"\nuse rule kept_rate from \"@pricing/rules/vat_rate.rule\"\n\nfn vat(subtotal: money[GBPc] range >=0GBPc <=10_000GBPc, category: tax_rate.category) -> money[GBPc]\n  description \"The VAT on the subtotal, at the rate of the module tax\"\n  half_up(subtotal × tax_rate(category: category).rate, 1GBPc)\n\nfn vat_kept(subtotal: money[GBPc] range >=0GBPc <=10_000GBPc, category: kept_rate.category) -> money[GBPc]\n  description \"The VAT on the subtotal, at the rate the module pricing keeps\"\n  half_up(subtotal × kept_rate(category: category).rate, 1GBPc)\n\nexamples\n  vat(subtotal: 1000GBPc, category: tax_rate.category.standard) = 200GBPc\n  vat_kept(subtotal: 1000GBPc, category: kept_rate.category.reduced) = 50GBPc\n";

// ── the names that are one at run time ──────────────────────────────────────────────────────────

/// A flow of `pricing`, run by a flow of the module as its child, makes the Temporal workflow type
/// the module's own `place_order.flow` makes: a worker knows a workflow by its type, so the two
/// would be one, and gen names both files and stops (exit 2), in the text and in the JSON. With
/// another version, the type is another, and the child is started by it.
#[test]
fn a_workflow_type_two_modules_make_stops_gen() {
    let t = example();
    t.write("pricing/flows/place_order.flow", PRICING_FLOW);
    t.write("shop/orders/price_order.flow", PRICE_ORDER_FLOW);
    let shop = t.path().join("shop");
    let (code, stdout, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "check passes: the type is a name of the package's, {stdout}{err}");
    let out = t.path().join("out");
    let (code, stdout, err) = generate(&shop, t.path(), &out, &[]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert_eq!(
        err,
        "error: `@pricing/flows/place_order.flow` and `orders/place_order.flow` both make the Temporal workflow type `place_order_v1`: a workflow runs by the name of its type, so one package cannot hold both; if one is this module's, give it another name or version\n"
    );
    let (_, _, err) = generate(&shop, t.path(), &out, &["--lang", "ja"]);
    assert_eq!(
        err,
        "エラー: `@pricing/flows/place_order.flow` と `orders/place_order.flow` は、どちらも Temporal のワークフローの型 `place_order_v1` を作ります。ワークフローは型の名前で走るので、一つのパッケージに二つは置けません。手元のワークフローなら、名前か版を変えてください\n"
    );
    let (code, stdout, _) = generate(&shop, t.path(), &out, &["--format", "json"]);
    let v = json(&stdout);
    assert_eq!((code, &v["error"]["code"], &v["files"]), (2, &serde_json::json!(2), &serde_json::json!([])));
    assert!(!out.exists(), "nothing is written");
    // another version: another type
    replace(&t, "pricing/flows/place_order.flow", "workflow place_order v1", "workflow place_order v2");
    let (code, _, err) = generate(&shop, t.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
    assert_has(&out.join("typescript/flows/price_order/workflow.ts"), "executeChild(\"place_order_v2\", ");
    assert!(out.join("typescript/deps/pricing/flows/place_order/workflow.ts").is_file());
    assert!(out.join("go/deps/pricing/flows/place_order/workflow.go").is_file());
}

/// A flow of `pricing` that prices an order by `pricing`'s shipping fee.
const PRICING_FLOW: &str = "# The price of an order, as a workflow of its own that the shop runs as its child.\nworkflow place_order v1\ndescription \"Prices an order: its shipping fee by the rule shipping_fee, for the shop's flow that runs it as its child\"\n\nuse rule shipping from \"../rules/shipping_fee.rule\"\n\ninputs\n  weight : mass[g]  range >=1g <=30_000g\n  region : shipping.region\n\noutputs\n  fee : money[GBPc]\n\nflow\n  let parcel = shipping(weight: weight, region: region)\n  succeed fee = parcel.fee\n";

/// A flow of the module that runs `pricing`'s as its child.
const PRICE_ORDER_FLOW: &str = "# The price of an order, from the workflow of the module pricing, run as a child.\nworkflow price_order v1\ndescription \"Prices an order by the workflow of the module pricing, run as its child\"\n\nuse rule shipping from \"@pricing/rules/shipping_fee.rule\"\n\nrecord Priced\n  fee : money[GBPc]\n\ninputs\n  weight : mass[g]  range >=1g <=30_000g\n  region : shipping.region\n\noutputs\n  fee : money[GBPc]\n\ntask price(weight: mass[g]  range >=1g <=30_000g, region: shipping.region) -> Priced\n  flow \"@pricing/flows/place_order.flow\"\n\nflow\n  let p = price(weight: weight, region: region)\n  succeed fee = p.fee\n";

/// The gate of the module's desk reads the gate of `pricing`'s staff with `use gate`: one namespace,
/// which sekisho holds the two files to, and the package holds both gates, `pricing`'s Cedar under
/// `cedar/deps/pricing/`. A gate of the module's counter, in the namespace `Shop` but not read with
/// `pricing`'s, declares `Shop::Role` too: Cedar knows a type by its name in its namespace, so gen
/// names both files and stops (exit 2).
#[test]
fn a_name_of_cedars_two_modules_declare_stops_gen() {
    let t = example();
    t.write("pricing/authz/staff.gate", "gate staff v1\ndescription \"The staff of the Acme shop who may change a price\"\nnamespace Shop\n\nrole pricer\n  description \"Sets the prices of the shop\"\n");
    t.write("shop/authz/desk.gate", "gate desk v1\ndescription \"Who may serve at the shop's desk, beside the staff of the module pricing\"\nnamespace Shop\n\nuse gate \"@pricing/authz/staff.gate\"\n\nrole clerk\n  description \"Serves customers at the desk\"\n");
    let shop = t.path().join("shop");
    let out = t.path().join("out");
    let (code, _, err) = generate(&shop, t.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
    let gates: Vec<String> = files(&out).into_iter().filter(|f| f.starts_with("cedar/") || f.contains("authz/")).collect();
    assert_eq!(
        gates,
        [
            "cedar/deps/pricing/staff.cedar",
            "cedar/deps/pricing/staff.cedarschema",
            "cedar/deps/pricing/staff.cedarschema.json",
            "cedar/deps/pricing/staff.policies.json",
            "cedar/desk.cedar",
            "cedar/desk.cedarschema",
            "cedar/desk.cedarschema.json",
            "cedar/desk.policies.json",
            "go/authz/desk/desk.go",
            "go/deps/pricing/authz/staff/staff.go",
            "python/generated/authz/__init__.py",
            "python/generated/authz/desk.py",
            "python/generated/deps/pricing/authz/__init__.py",
            "python/generated/deps/pricing/authz/staff.py",
            "typescript/authz/desk.ts",
            "typescript/authz/index.ts",
            "typescript/deps/pricing/authz/index.ts",
            "typescript/deps/pricing/authz/staff.ts"
        ]
    );
    assert_has(&out.join("cedar/deps/pricing/staff.cedar"), "// Source: @pricing/authz/staff.gate (gate staff v1, sha256:");
    assert_has(&out.join("cedar/deps/pricing/staff.cedar"), ", module example.com/acme/pricing 1.1.0)\n");
    // a gate of the module in pricing's namespace, not read with pricing's
    t.write("shop/authz/counter.gate", "gate counter v1\ndescription \"Who may take payments at the shop's counter\"\nnamespace Shop\n\nrole cashier\n  description \"Takes payments at the counter\"\n");
    let (code, stdout, err) = generate(&shop, t.path(), &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert_eq!(
        err,
        "error: `@pricing/authz/staff.gate` and `authz/counter.gate` both declare the Cedar `Shop::Role`: Cedar knows a type or an action by its name in its namespace, so one package cannot hold both; if one is this module's gate, give it another `namespace` or name\n"
    );
    let (_, _, err) = generate(&shop, t.path(), &out, &["--lang", "ja"]);
    assert_eq!(
        err,
        "エラー: `@pricing/authz/staff.gate` と `authz/counter.gate` は、どちらも Cedar の `Shop::Role` を宣言しています。Cedar は型と操作を名前空間の中の名前で知るので、一つのパッケージに二つは置けません。手元のゲートなら、`namespace` か名前を変えてください\n"
    );
    // in its own namespace, it is the module's own
    replace(&t, "shop/authz/counter.gate", "namespace Shop", "namespace Counter");
    let (code, _, err) = generate(&shop, t.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
}

// ── check and gen alike ─────────────────────────────────────────────────────────────────────────

/// The module's contribution `spring_sale` adds to `pricing`'s open rule `order_discount`, which
/// its `to` reaches: the rule's part in the package is the rule with the contribution, as the check
/// of the whole module reads it. Where the check fails, gen does not generate, and names the same
/// files: the contribution pinned at a shape the rule no longer has, or with a term it does not
/// declare (the error is the module's, and the rule of `pricing` is not a file that fails, DESIGN
/// 17.8), and an example of `pricing`'s that does not hold. A file of `pricing` that the package
/// does not hold is not checked by gen: `ritsu check` of the whole module says it.
#[test]
fn a_contribution_to_a_dependencys_rule_is_answered_alike_by_check_and_gen() {
    let t = example();
    t.write("pricing/rules/order_discount.rule", ORDER_DISCOUNT);
    t.write("shop/rules/spring_sale.rule", SPRING_SALE);
    let shop = t.path().join("shop");
    let out = t.path().join("out");
    let checked = |args: &[&str]| -> i32 { ritsu_in(&shop, t.path(), &[&["check"][..], args].concat()).0 };
    let generated = |args: &[&str]| -> (i32, String, String) { generate(&shop, t.path(), &out, args) };
    assert_eq!(checked(&[]), 0);
    let (code, _, err) = generated(&[]);
    assert_eq!(code, 0, "{err}");
    let rule = out.join("typescript/deps/pricing/rules/order_discount.ts");
    assert_has(&rule, "// Contributions: spring_sale = rules/spring_sale.rule (signature:67a223ec46dce359, sha256:");
    assert_has(&rule, "const spring_sale_sale_off = 10000n; // definition\n");
    assert!(!files(&out).iter().any(|f| f.contains("spring_sale")), "a contribution has no part of its own");
    // the shape it was pinned at has changed
    replace(&t, "shop/rules/spring_sale.rule", "signature:67a223ec46dce359", "signature:0000000000000000");
    assert_eq!(checked(&[]), 1);
    let (code, stdout, err) = generated(&[]);
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert!(err.contains("error[rulec E040]: The shape of `@pricing/rules/order_discount.rule` this contribution relies on has changed\n  --> rules/spring_sale.rule:4\n"), "{err}");
    assert!(err.ends_with("error: 1 file(s) do not pass check, so nothing is generated: rules/spring_sale.rule\n"), "{err}");
    let (code, stdout, _) = generated(&["--format", "json"]);
    let v = json(&stdout);
    assert_eq!((code, &v["error"]["code"]), (1, &serde_json::json!(1)));
    let codes: Vec<(&str, &str)> = v["diagnostics"].as_array().unwrap().iter().map(|d| (d["code"].as_str().unwrap(), d["file"].as_str().unwrap())).collect();
    assert_eq!(codes, [("E040", "rules/spring_sale.rule")], "the error is the module's, as `ritsu check` files it");
    let (code, _, err) = generated(&["--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(err.ends_with("検査を通らないファイルが 1 個あるので、何も生成しません: rules/spring_sale.rule\n"), "{err}");
    replace(&t, "shop/rules/spring_sale.rule", "signature:0000000000000000", "signature:67a223ec46dce359");
    // a term the contribution does not declare: check fails the module's file and passes the
    // dependency's, and gen names the one file with the same error, with the note that names the rule
    replace(&t, "shop/rules/spring_sale.rule", "  sale_off\n", "  sale_offf\n");
    let (code, out, _) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 1);
    assert!(out.ends_with("dependencies: 2 modules, 4 files (rulec 4): all pass\nritsu check: 3 files (rulec 1, kaname 1, dandori 1): 1 fail (1 error); borders between the languages: 0 checked, 0 undecided\n"), "{out}");
    let (code, stdout, err) = generated(&[]);
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert!(err.starts_with("error[rulec E067]: The term sale_offf is not declared\n  --> rules/spring_sale.rule:9\n"), "{err}");
    assert!(err.contains(" This line adds to `@pricing/rules/order_discount.rule`, a rule of the dependency `@pricing`, and the check of that rule found it.\n"), "{err}");
    assert!(err.ends_with("error: 1 file(s) do not pass check, so nothing is generated: rules/spring_sale.rule\n"), "{err}");
    let (code, stdout, _) = generated(&["--format", "json"]);
    let v = json(&stdout);
    assert_eq!(code, 1);
    let codes: Vec<(&str, &str)> = v["diagnostics"].as_array().unwrap().iter().map(|d| (d["code"].as_str().unwrap(), d["file"].as_str().unwrap())).collect();
    assert_eq!(codes, [("E067", "rules/spring_sale.rule")]);
    let (code, _, err) = generated(&["--lang", "ja"]);
    assert_eq!(code, 1);
    assert!(err.contains(" この行は、依存 `@pricing` の規則 `@pricing/rules/order_discount.rule` に足すものです。その規則の検査で見つかりました。\n"), "{err}");
    assert!(err.ends_with("検査を通らないファイルが 1 個あるので、何も生成しません: rules/spring_sale.rule\n"), "{err}");
    replace(&t, "shop/rules/spring_sale.rule", "  sale_offf\n", "  sale_off\n");
    // an example of pricing's that does not hold, in a file the package holds
    replace(&t, "pricing/rules/shipping_fee.rule", "| 2500g   | domestic | 395GBPc  |", "| 2500g   | domestic | 999GBPc  |");
    assert_eq!(checked(&[]), 1);
    let (code, _, err) = generated(&[]);
    assert_eq!(code, 1);
    assert!(err.ends_with("error: 2 file(s) do not pass check, so nothing is generated: @pricing/rules/shipping_fee.rule, orders/place_order.flow\n"), "{err}");
    replace(&t, "pricing/rules/shipping_fee.rule", "| 2500g   | domestic | 999GBPc  |", "| 2500g   | domestic | 395GBPc  |");
    assert_eq!(checked(&[]), 0);
    // in a file of pricing's that the package does not hold
    t.write("pricing/rules/gift_wrap.rule", t.read("pricing/rules/shipping_fee.rule").replacen("rule shipping_fee v1", "rule gift_wrap v1", 1).replace("| 2500g   | domestic | 395GBPc  |", "| 2500g   | domestic | 999GBPc  |"));
    assert_eq!(checked(&[]), 1);
    let (code, _, err) = generated(&[]);
    assert_eq!(code, 0, "{err}");
}

/// An open rule of `pricing`'s, which the campaigns of the modules that use it add to.
const ORDER_DISCOUNT: &str = "# The discount of an order: what the campaigns of the shop add, up to its subtotal.\nrule order_discount v1\ndescription \"The discount of an order: the sum of what the campaigns add, up to the subtotal\"\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=2_000_000GBPc\n\noutputs\n  discount : money[GBPc]  round down(1GBPc)\n\n# What the campaigns take off, from files of their own: nothing when none does.\nopen campaigns : money[GBPc]  combine sum  empty 0GBPc  range >=0GBPc <=10_000GBPc\n\ndefine discount : money[GBPc] = min(campaigns, subtotal)\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 0GBPc       |\n";

/// The module's campaign, added to `pricing`'s `order_discount`.
const SPRING_SALE: &str = "# A campaign of the shop, added to the rule order_discount of the module pricing.\ncontribution spring_sale v1\ndescription \"A spring sale: 100 pence off every order\"\nto \"@pricing/rules/order_discount.rule\" signature:67a223ec46dce359\n\ndefine sale_off : money[GBPc] = 100GBPc\n\ninto campaigns\n  sale_off\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 100GBPc     |\n| 50GBPc   | 50GBPc      |\n";

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

/// The store `store` with the module under `dir` placed in it, and its lock line.
fn put(store: &Store, dir: &Path) -> Locked {
    store::put_directory(store, dir, "2026-10-28", &[]).unwrap_or_else(|e| panic!("{}", e.en)).locked()
}

/// `pricing` and `tax` from a registry, placed in the store under `t`, and `shop`'s lock, written by
/// the ritsu `ritsu`.
fn from_a_registry(t: &TempDir, ritsu: &str) -> (Locked, Locked) {
    replace(t, "pricing/ritsu.mod", "require tax path \"../tax\"", "require tax oci \"example.com/acme/tax\" 1.0.0");
    replace(t, "shop/ritsu.mod", "require pricing path \"../pricing\"", "require pricing oci \"example.com/acme/pricing\" 1.1.0");
    let store = Store::at(t.path().join("store"));
    let tax = put(&store, &t.path().join("tax"));
    let pricing = put(&store, &t.path().join("pricing"));
    write_lock(t, vec![pricing.clone(), tax.clone()], ritsu);
    (pricing, tax)
}

fn write_lock(t: &TempDir, lines: Vec<Locked>, ritsu: &str) {
    let mut l = Lock::new(lines);
    l.ritsu = v(ritsu);
    t.write("shop/ritsu.lock", l.text());
}

/// The tree is read as `ritsu check` reads it: its errors are said, and nothing is generated (exit
/// 1; the JSON has them in its diagnostics); what stops the run is said on the standard error, with
/// no JSON (exit 2).
#[test]
fn the_tree_stops_gen_as_it_stops_check() {
    let t = example();
    let (pricing, _) = from_a_registry(&t, env!("CARGO_PKG_VERSION"));
    let shop = t.path().join("shop");
    let store = t.path().join("store");
    let out = t.path().join("out");
    let (code, _, err) = generate(&shop, &store, &out, &[]);
    assert_eq!(code, 0, "read from the store: {err}");
    // a lock with no line for tax
    write_lock(&t, vec![pricing], env!("CARGO_PKG_VERSION"));
    let (code, stdout, err) = generate(&shop, &store, &out, &[]);
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert_eq!(
        err,
        "error[ritsu E304]: @pricing/ritsu.mod:7:9: `ritsu.lock` does not match `ritsu.mod`: the lock has no line for `require tax oci \"example.com/acme/tax\" 1.0.0`; run `ritsu mod lock`\n     7 | require tax oci \"example.com/acme/tax\" 1.0.0\nerror: the modules could not be read (1 error), so nothing is generated\n"
    );
    let (_, _, err) = generate(&shop, &store, &out, &["--lang", "ja"]);
    assert!(err.ends_with("エラー: モジュールを読めません（エラー 1 件）。何も生成しません\n"), "{err}");
    let (code, stdout, err) = generate(&shop, &store, &out, &["--format", "json"]);
    assert_eq!((code, err.as_str()), (1, ""));
    let v = json(&stdout);
    assert_eq!((&v["error"]["code"], &v["error"]["message"], &v["files"]), (&serde_json::json!(1), &serde_json::json!("the modules could not be read (1 error), so nothing is generated"), &serde_json::json!([])));
    assert_eq!((v["diagnostics"][0]["code"].as_str(), v["diagnostics"][0]["file"].as_str()), (Some("E304"), Some("@pricing/ritsu.mod")));
    // a store without the modules
    let empty = TempDir::new("modules-gen-empty-store");
    for more in [&[][..], &["--format", "json"][..]] {
        let (code, stdout, err) = generate(&shop, empty.path(), &out, more);
        assert_eq!((code, stdout.as_str()), (2, ""), "{more:?}");
        assert_eq!(err, "error: the dependency `example.com/acme/pricing` 1.1.0 is neither in the store nor in this module's copy of it; run `ritsu mod fetch`\n");
    }
}

/// The `ritsu` line of the lock is a record (DESIGN 17.4): `gen --check` does not compare it, and
/// where the package differs, a note says the ritsu that wrote the lock is another, which may be
/// why. The exit code is the comparison's.
#[test]
fn gen_check_notes_the_ritsu_that_wrote_the_lock() {
    let t = example();
    from_a_registry(&t, "0.1.0");
    let shop = t.path().join("shop");
    let store = t.path().join("store");
    let out = t.path().join("out");
    let o = out.display().to_string();
    let (code, _, err) = generate(&shop, &store, &out, &[]);
    assert_eq!(code, 0, "{err}");
    let (code, stdout, _) = generate(&shop, &store, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""), "nothing differs: no note");
    let part = out.join("typescript/deps/pricing/rules/order_total.ts");
    std::fs::write(&part, read(&part).replace("standard", "Standard")).unwrap();
    let now = env!("CARGO_PKG_VERSION");
    let (code, stdout, _) = generate(&shop, &store, &out, &["--check"]);
    assert_eq!(code, 1);
    assert_eq!(
        stdout,
        format!("generated file is stale or hand-edited: {o}/typescript/deps/pricing/rules/order_total.ts\nnote: ritsu.lock was written by ritsu 0.1.0, and this is ritsu {now}: what differs may come from the two versions\n")
    );
    let (_, stdout, _) = generate(&shop, &store, &out, &["--check", "--lang", "ja"]);
    assert!(stdout.ends_with(&format!("備考: ritsu.lock を書いた ritsu は 0.1.0 で、いまの ritsu は {now} です。生成物の違いは、版の違いから来ているかもしれません\n")), "{stdout}");
    // the lock this ritsu wrote: no note
    let lock = t.read("shop/ritsu.lock");
    t.write("shop/ritsu.lock", lock.replace("ritsu 0.1.0", &format!("ritsu {now}")));
    let (code, stdout, _) = generate(&shop, &store, &out, &["--check"]);
    assert_eq!((code, stdout.as_str()), (1, format!("generated file is stale or hand-edited: {o}/typescript/deps/pricing/rules/order_total.ts\n").as_str()));
}

/// A dependency's name is the name of a Python package of the package's (`deps/lambda/`), which a
/// word Python keeps cannot be: gen of the Python says so and stops (exit 2), and the other
/// languages are written. With no `--target`, gen makes sure of it before it writes any target, and
/// writes none.
#[test]
fn a_dependency_named_with_a_word_python_keeps_has_no_python() {
    let t = example();
    t.write("till/ritsu.mod", "# A till of the shop, which requires the module tax by a word Python keeps.\nmodule example.com/acme/till\nversion 0.1.0\n\nrequire lambda path \"../tax\"\n");
    t.write("till/vat.kn", TILL_KN);
    let till = t.path().join("till");
    let out = t.path().join("out");
    for more in [&[][..], &["--check"][..]] {
        let (code, stdout, err) = generate(&till, t.path(), &out, more);
        assert_eq!((code, stdout.as_str()), (2, ""), "{more:?}");
        assert_eq!(err, "error: the dependency `lambda` is named with a word Python keeps, which the package cannot hold in its deps/; require it by another name\n");
        assert!(!out.exists(), "{more:?}: no target is written");
    }
    let (code, stdout, _) = generate(&till, t.path(), &out, &["--format", "json"]);
    let v = json(&stdout);
    assert_eq!((code, &v["error"]["code"], &v["files"]), (2, &serde_json::json!(2), &serde_json::json!([])));
    assert!(!out.exists(), "the JSON: no target is written");
    let (code, stdout, err) = generate(&till, t.path(), &out, &["--target", "python"]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert_eq!(err, "error: the dependency `lambda` is named with a word Python keeps, which the package cannot hold in its deps/; require it by another name\n");
    let (_, _, err) = generate(&till, t.path(), &out, &["--target", "python", "--lang", "ja"]);
    assert_eq!(err, "エラー: 依存の名前 `lambda` は Python が予約している語なので、パッケージの deps/ に置けません。`require` で別の名前を付けてください\n");
    for target in ["typescript", "go"] {
        let (code, _, err) = generate(&till, t.path(), &out, &["--target", target]);
        assert_eq!(code, 0, "{target}: {err}");
    }
    assert!(out.join("typescript/deps/lambda/rules/vat_rate.ts").is_file());
    assert!(out.join("go/deps/lambda/rules/vatrate/vat_rate.go").is_file());
}

/// A file of a dependency given on the command line (`@pricing/…`, DESIGN 17.6) is made into the
/// package with what it reaches, each under its dependency's `deps/`, and nothing of the module's
/// own; a path that starts with `@` and names no file is said as a path written in a file is (exit
/// 2), and nothing is written.
#[test]
fn gen_of_a_dependencys_file_given_on_the_command_line() {
    let t = example();
    let shop = t.path().join("shop");
    let out = t.path().join("out");
    let (code, _, err) = generate(&shop, t.path(), &out, &["--target", "typescript", "@pricing/rules/order_total.rule"]);
    assert_eq!(code, 0, "{err}");
    assert_has(&out.join("typescript/deps/pricing/rules/order_total.ts"), "// Source: @pricing/rules/order_total.rule (rule order_total v1, sha256:");
    assert!(out.join("typescript/deps/pricing/deps/tax/rules/vat_rate.ts").is_file(), "what the rule applies goes with it");
    assert!(!out.join("typescript/flows").exists() && !out.join("typescript/glue").exists(), "nothing of the module's own");
    std::fs::remove_dir_all(&out).unwrap();
    let (code, stdout, err) = generate(&shop, t.path(), &out, &["@pricing/rules/fee.rule"]);
    assert_eq!((code, stdout.as_str()), (2, ""));
    assert_eq!(err, "error: the dependency `@pricing` (`example.com/acme/pricing`, `../pricing`) has no `rules/fee.rule`\n");
    let (_, _, err) = generate(&shop, t.path(), &out, &["@pricing/rules/fee.rule", "--lang", "ja"]);
    assert_eq!(err, "エラー: 依存 `@pricing`（`example.com/acme/pricing`、`../pricing`）に `rules/fee.rule` はありません\n");
    let (code, stdout, _) = generate(&shop, t.path(), &out, &["@tax/rules/vat_rate.rule", "--format", "json"]);
    let v = json(&stdout);
    assert_eq!((code, &v["error"]["code"], &v["files"]), (2, &serde_json::json!(2), &serde_json::json!([])));
    assert_eq!(
        v["error"]["message"],
        serde_json::json!("the `@tax` of `@tax/rules/vat_rate.rule` is no dependency: the `ritsu.mod` of the module `example.com/acme/shop` has no `require tax …`")
    );
    assert!(!out.exists(), "nothing is written");
}

/// A function of the till, which reads the rate of the module it requires as `lambda`.
const TILL_KN: &str = "# The VAT on a sale at the till, at the rate of the module tax, which the till requires as `lambda`.\nmodule till v1\ndescription \"The VAT on a sale's subtotal, at the rate of the module tax\"\n\nuse rule vat_rate from \"@lambda/rules/vat_rate.rule\"\n\nfn vat(subtotal: money[GBPc] range >=0GBPc <=10_000GBPc, category: vat_rate.category) -> money[GBPc]\n  description \"The VAT on the subtotal\"\n  half_up(subtotal × vat_rate(category: category).rate, 1GBPc)\n\nexamples\n  vat(subtotal: 1000GBPc, category: vat_rate.category.standard) = 200GBPc\n";

// ── a .proto of the module that imports a dependency's ──────────────────────────────────────────

/// `pricing`'s money, as the services of the shop carry it, under `proto/` where its package says.
const MONEY: &str = "// An amount of money as the services of the Acme shop carry it: pence, and the currency.\nsyntax = \"proto3\";\n\npackage acme.pricing.v1;\n\nmessage Money {\n  int32 pence = 1;\n  string currency = 2;\n}\n";

/// The module's quote service, whose total is `pricing`'s money, imported by the path of its package.
const QUOTES: &str = "// The shop's quotes, whose totals are the Money of the module pricing: imported by the path of its\n// package, as the dependency lays it out.\nsyntax = \"proto3\";\n\npackage acme.shop.v1;\n\nimport \"acme/pricing/v1/money.proto\";\n\nservice QuoteService {\n  rpc Quote(QuoteRequest) returns (QuoteResponse);\n}\n\nmessage QuoteRequest {\n  string order_id = 1;\n}\n\nmessage QuoteResponse {\n  acme.pricing.v1.Money total = 1;\n}\n";

/// A flow of the module that asks the quote service.
const QUOTE_FLOW: &str = "# A quote of an order, from the shop's quote service, whose total is the Money of the module pricing.\nworkflow quote v1\ndescription \"Asks the quote service for the total of an order, in the money of the module pricing\"\n\nuse proto quotes from \"../proto/acme/shop/v1/quotes.proto\"\n  url \"https://quotes.example.com\"\n\ninputs\n  order_id : string\n\noutputs\n  pence : int\n\ntask quote(order_id: string) -> quotes.QuoteResponse\n  connect quotes \"QuoteService/Quote\"\n  idempotent\n\nflow\n  let q = quote(order_id: order_id)\n  match q.total\n    none => fail NoTotal \"the quote of {order_id} has no total\"\n    some total => succeed pence = total.pence\n";

/// The module's `quotes.proto` imports `pricing`'s `money.proto` by the path of its package
/// (DESIGN 17.6): the flow that asks the quote service reads `pricing`'s `Money` through it, and the
/// package's types hold it. The roots one run of protoc is given for the file are the module's, then
/// `pricing`'s, each the directory its files are read from, and at the tools level protoc builds the
/// file from them. A file of the module at the path of `pricing`'s is the module's own, and is read
/// first.
#[test]
fn a_proto_of_the_module_imports_a_dependencys_by_its_package() {
    let t = example();
    t.write("pricing/proto/acme/pricing/v1/money.proto", MONEY);
    t.write("shop/proto/acme/shop/v1/quotes.proto", QUOTES);
    t.write("shop/quotes/quote.flow", QUOTE_FLOW);
    let shop = t.path().join("shop");
    let (code, stdout, err) = ritsu_in(&shop, t.path(), &["check"]);
    assert_eq!(code, 0, "{stdout}{err}");
    assert!(stdout.contains("dependencies: 2 modules, 4 files (rulec 3, proto 1): all pass\n"), "pricing's money is read with its module: {stdout}");
    let out = t.path().join("out");
    let (code, _, err) = generate(&shop, t.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
    assert_has(&out.join("typescript/flows/quote/types.ts"), "export interface quotes_acme_pricing_v1_Money {\n  \"pence\": number;\n  \"currency\": string;\n}\n");
    assert_has(&out.join("python/generated/flows/quote/types.py"), "quotes_acme_pricing_v1_Money = TypedDict(\"quotes_acme_pricing_v1_Money\", {\"pence\": \"int\", \"currency\": \"str\"})\n");
    assert_has(&out.join("go/flows/quote/types.go"), "type QuotesAcmePricingV1Money struct {\n");
    // the roots of one run of protoc
    let p = ritsu_proto::protoc_roots(&shop, "proto/acme/shop/v1/quotes.proto", &[]).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(p.entry, "acme/shop/v1/quotes.proto");
    assert_eq!(p.includes, [shop.join("proto"), t.path().join("pricing/proto")]);
    if ready(Need::Protoc, have_protoc, "protoc is not installed; the .proto of the module is not built from the roots ritsu reads it from") {
        let set = t.path().join("quotes.pb");
        let mut c = Command::new("protoc");
        for i in &p.includes {
            c.arg("-I").arg(i);
        }
        let o = c.arg("--include_imports").arg(format!("--descriptor_set_out={}", set.display())).arg(&p.entry).current_dir(&shop).output().unwrap();
        assert!(o.status.success(), "protoc refuses quotes.proto:\n{}", String::from_utf8_lossy(&o.stderr));
        let bytes = std::fs::read(&set).unwrap();
        let has = |name: &str| bytes.windows(name.len()).any(|w| w == name.as_bytes());
        assert!(has("acme/pricing/v1/money.proto") && has("acme/shop/v1/quotes.proto"), "the set holds both files by the names their imports give them");
        println!("protoc builds quotes.proto, which imports the money of pricing");
    }
    // the module's own file at that path
    t.write("shop/proto/acme/pricing/v1/money.proto", MONEY.replace("  string currency = 2;\n", "  string region = 2;\n"));
    let (code, _, err) = generate(&shop, t.path(), &out, &["--target", "typescript"]);
    assert_eq!(code, 0, "{err}");
    assert_has(&out.join("typescript/flows/quote/types.ts"), "  \"pence\": number;\n  \"region\": string;\n");
    let p = ritsu_proto::protoc_roots(&shop, "proto/acme/shop/v1/quotes.proto", &[]).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(p.includes, [shop.join("proto")], "the module's own file is the one read");
}

fn have_protoc() -> bool {
    Command::new("protoc").arg("--version").output().is_ok_and(|o| o.status.success())
}

// ── the type checkers ────────────────────────────────────────────────────────────────────────────

/// The packages of modules pass `tsc --strict`, `mypy --strict` and `go vet`: the example `shop`,
/// `店`, and `shop` with the material of the tests above that writes code (two files of one name,
/// the child of a dependency's flow, the contribution, the quotes of the `.proto`), each part
/// importing the others by the paths of the package's directories. The code of the gates is held
/// to the type checkers by tests/gen.rs, with the Cedar each language asks.
#[test]
fn the_packages_of_modules_pass_the_type_checkers() {
    let empty = TempDir::new("modules-gen-types-no-store");
    let t = TempDir::new("modules-gen-types");
    let mut made = Vec::new();
    for (dir, name) in [("tests/projects/modules/shop", "shop"), ("tests/projects/modules.ja/店", "店")] {
        let out = t.path().join(name);
        let (code, _, err) = generate(&here().join(dir), empty.path(), &out, &[]);
        assert_eq!(code, 0, "{name}: {err}");
        made.push(out);
    }
    let all = example();
    replace(&all, "shop/ritsu.mod", "require pricing path \"../pricing\"\n", "require pricing path \"../pricing\"\nrequire tax path \"../tax\"\n");
    all.write("pricing/rules/vat_rate.rule", all.read("tax/rules/vat_rate.rule").replace("The rates are the example's own\"", "The rates pricing keeps by itself, the example's own\""));
    all.write("shop/checkout/vat.kn", VAT_KN);
    all.write("pricing/flows/place_order.flow", PRICING_FLOW.replace("workflow place_order v1", "workflow place_order v2"));
    all.write("shop/orders/price_order.flow", PRICE_ORDER_FLOW);
    all.write("pricing/rules/order_discount.rule", ORDER_DISCOUNT);
    all.write("shop/rules/spring_sale.rule", SPRING_SALE);
    all.write("pricing/proto/acme/pricing/v1/money.proto", MONEY);
    all.write("shop/proto/acme/shop/v1/quotes.proto", QUOTES);
    all.write("shop/quotes/quote.flow", QUOTE_FLOW);
    let out = t.path().join("all");
    let (code, _, err) = generate(&all.path().join("shop"), all.path(), &out, &[]);
    assert_eq!(code, 0, "{err}");
    made.push(out);
    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                for out in &made {
                    type_check_typescript(&out.join("typescript"), &tsc, &modules);
                }
                println!("tsc --strict passes the TypeScript of {} packages of modules", made.len());
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) or tigerbeetle-node (crates/chobo/tools/runner) is not installed; the TypeScript of the packages of modules is not type-checked"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        match mypy() {
            Some((mypy, python, tigerbeetle)) => {
                for out in &made {
                    type_check_python(&out.join("python"), &mypy, &python, std::slice::from_ref(&tigerbeetle));
                }
                println!("mypy --strict passes the Python of {} packages of modules", made.len());
            }
            None => ritsu_testkit::skip("mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python) or tigerbeetle (crates/chobo/tools/runner) is not installed; the Python of the packages of modules is not type-checked"),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the Go of the packages of modules is not vetted") {
        match go_module() {
            Some(gomod) => {
                for out in &made {
                    let name = out.file_name().unwrap().to_string_lossy().to_string();
                    type_check_go(&t.path().join(format!("go-{name}")), &out.join("go"), &gomod);
                }
                println!("go vet and gofmt pass the Go of {} packages of modules", made.len());
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go of the packages of modules is not vetted"),
        }
    }
}
