//! `ritsu gen --library` (DESIGN 17.16; PLAN E.5): the example's `tax`, `pricing` and `shop`, each
//! with `library` lines added to a copy of its ritsu.mod, written as the libraries their registries
//! take. `shop` reads `pricing`'s rules and its type of money from `pricing`'s library, by the
//! library's name; `pricing` applies `tax`'s rule and imports nothing of it, so its manifests do
//! not name `tax`. The golden files are each library's files, and the whole of what ritsu writes
//! beside the parts: the manifests, the indexes and the units module. Beside them: a dependency
//! with no `library` line, which stops the run unless `--embed` puts it in; the arguments a library
//! refuses, and what it says of each in English and in Japanese; a version from 2 in the Go module's
//! path; and what the registries' tools write beside a library, which `--check` and the next run
//! leave alone. And the pages on publishing libraries (website/docs/libraries.md and the Japanese
//! one) show what ritsu writes and says.
//!
//! At the tools level, the three are built as their registries' tools build them (tsc into dist/
//! as npm's `prepare` does, a wheel by setuptools, a Go module required by its path and version),
//! each installed where the next and the host find it, and a host that uses `pricing` and `shop`
//! passes `tsc --strict`, `mypy --strict` and `go vet`, and runs: an amount of money `shop` gives
//! is one `pricing` takes, and a mass is not. The tools are those tests/type_checkers takes from
//! the languages' tests; nothing is fetched.
//!
//! The material is tests/projects/modules, copied. The golden files are tests/golden/library/
//! (`RITSU_BLESS=1` writes them).

mod common;
mod type_checkers;

use ritsu_base::modules::store;
use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;
use type_checkers::*;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The `library` lines of the example's modules: TypeScript's, Python's, Go's.
const LINES: [(&str, &str, &str, &str); 3] = [
    ("tax", "@acme/tax", "acme-tax", "example.com/acme/tax-go"),
    ("pricing", "@acme/pricing", "acme-pricing", "example.com/acme/pricing-go"),
    ("shop", "@acme/shop", "acme-shop", "example.com/acme/shop-go"),
];

/// The example's modules, copied, the ritsu.mod of each that `with` names with its `library` lines.
fn example(with: &[&str]) -> TempDir {
    let t = TempDir::new("library");
    for (m, ts, py, go) in LINES {
        ritsu_testkit::tmp::copy_dir(&here().join("tests/projects/modules").join(m), &t.path().join(m));
        if with.contains(&m) {
            let rel = format!("{m}/ritsu.mod");
            t.write(&rel, format!("{}\nlibrary typescript \"{ts}\"\nlibrary python \"{py}\"\nlibrary go \"{go}\"\n", t.read(&rel)));
        }
    }
    t
}

/// `ritsu` run in `dir` of `t`, with the store under `t` and no language asked of the environment:
/// the exit code, what it printed and what it said on stderr, `t`'s path as `<dir>`.
fn ritsu_in(t: &TempDir, dir: &str, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(t.path().join(dir)).args(args).env(store::VARIABLE, t.path().join("store"));
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG", "CHOBO_LANG", "GEAS_LANG", "YUEN_LANG", "SAKAI_LANG", "SEKISHO_LANG", "KANAME_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    let shown = |b: &[u8]| String::from_utf8_lossy(b).replace(&t.path().display().to_string(), "<dir>");
    (o.status.code().unwrap_or(-1), shown(&o.stdout), shown(&o.stderr))
}

/// `ritsu gen --library` of the module `m` of `t`, writing under `out/<m>` of `t`.
fn generate(t: &TempDir, m: &str, more: &[&str]) -> (i32, String, String) {
    let out = t.path().join("out").join(m);
    let mut args = vec!["gen", "--library", "--out", out.to_str().unwrap()];
    args.extend_from_slice(more);
    ritsu_in(t, m, &args)
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

fn assert_has(file: &Path, what: &str) {
    let text = read(file);
    assert!(text.contains(what), "{} has no\n{what}\nin:\n{text}", file.display());
}

fn golden(path: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join(path), got) {
        failures.push(e);
    }
}

/// A library under `out`: each language's files, then the whole of each file ritsu writes beside
/// the parts (the manifests, the indexes at the root, the units module, the declarations of the
/// modules loaded when asked), with ritsu's version as `<version>`.
fn library_shape(out: &Path) -> String {
    let whole = |f: &str| {
        ["package.json", "tsconfig.json", "optional-modules.d.ts", "index.ts", "units.ts", "pyproject.toml", "go.mod", "doc.go", "units/units.go"].contains(&f)
            || (f.matches('/').count() == 1 && (f.ends_with("/__init__.py") || f.ends_with("/units.py")))
    };
    let mut shape = String::new();
    let mut texts = String::new();
    for target in ["typescript", "python", "go"] {
        let all = files(&out.join(target));
        shape.push_str(&format!("{target}: {} files\n", all.len()));
        for f in &all {
            shape.push_str(&format!("  {f}\n"));
            if whole(f) {
                texts.push_str(&format!("\n── {target}/{f}\n{}", read(&out.join(target).join(f))));
            }
        }
    }
    (shape + &texts).replace(env!("CARGO_PKG_VERSION"), "<version>")
}

// ── the example as libraries ─────────────────────────────────────────────────────────────────────

/// `tax`, `pricing` and `shop` as libraries: none holds a dependency's parts under deps/; `shop`'s
/// parts import `pricing`'s by the library's name in each language, its units module hands on
/// `pricing`'s type of money, and its manifests ask for `pricing` by a range from the version the
/// tree chooses. `pricing` applies `tax`'s rule, and its manifests do not name `tax`. A library
/// just written is not stale.
#[test]
fn the_example_modules_are_written_as_libraries() {
    let t = example(&["tax", "pricing", "shop"]);
    let mut failures = Vec::new();
    for m in ["tax", "pricing", "shop"] {
        let (code, stdout, err) = generate(&t, m, &[]);
        assert_eq!(code, 0, "{m}: {stdout}{err}");
        let out = t.path().join("out").join(m);
        assert!(!files(&out).iter().any(|f| f.contains("deps/")), "{m}: a library holds no dependency's parts:\n{}", files(&out).join("\n"));
        golden(&format!("tests/golden/library/{m}.txt"), &library_shape(&out), &mut failures);
        let (code, stdout, _) = generate(&t, m, &["--check"]);
        assert_eq!((code, stdout.as_str()), (0, ""), "{m}: a library just written is not stale");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let shop = t.path().join("out/shop");
    assert_has(&shop.join("typescript/glue/basket.ts"), "import * as rule_pricing_order_total from \"@acme/pricing/rules/order_total\";\n");
    assert_has(&shop.join("typescript/flows/place_order/rules.ts"), "from \"@acme/pricing/rules/order_total\";\n");
    assert_has(&shop.join("typescript/units.ts"), "import type { GBPc } from \"@acme/pricing/units\";\n");
    assert_has(&shop.join("python/acme_shop/glue/basket.py"), "from acme_pricing.rules import order_total as _rule_pricing_order_total\n");
    assert_has(&shop.join("python/acme_shop/flows/place_order/rules.py"), "from acme_pricing.rules.order_total import ");
    assert_has(&shop.join("python/acme_shop/units.py"), "from acme_pricing.units import GBPc as GBPc  # money[GBPc]\n");
    assert_has(&shop.join("go/glue/basket/basket.go"), "pricingordertotal \"example.com/acme/pricing-go/rules/ordertotal\"\n");
    assert_has(&shop.join("go/flows/place_order/rules.go"), "\"example.com/acme/pricing-go/rules/ordertotal\"\n");
    assert_has(&shop.join("go/units/units.go"), "type GBPc = pricinggounits.GBPc\n");
    let pricing = t.path().join("out/pricing");
    for (f, tax) in [("typescript/package.json", "@acme/tax"), ("python/pyproject.toml", "acme-tax"), ("go/go.mod", "tax-go")] {
        assert!(!read(&pricing.join(f)).contains(tax), "pricing/{f} names {tax}, which pricing's code does not import");
    }
}

/// A dependency with no `library` line of a language stops the run: the library would have to hold
/// its parts, and does only when `--embed` names it, at deps/<name>/ as the app form holds them. A
/// module the package reaches only through a library is that library's to hold, and `--embed`
/// cannot take it. What is said is the golden file no-line.txt.
#[test]
fn a_dependency_with_no_library_line_stops_the_run_unless_it_is_embedded() {
    let t = example(&["pricing", "shop"]);
    let mut said = String::new();
    for lang in ["en", "ja"] {
        let (code, stdout, err) = generate(&t, "pricing", &["--lang", lang]);
        assert_eq!((code, stdout.as_str()), (2, ""), "{err}");
        said.push_str(&err);
    }
    let mut failures = Vec::new();
    golden("tests/golden/library/no-line.txt", &said, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(!t.exists("out/pricing"), "nothing is written");
    let (code, _, err) = generate(&t, "pricing", &["--embed", "tax"]);
    assert_eq!(code, 0, "{err}");
    let out = t.path().join("out/pricing");
    for f in ["typescript/deps/tax/rules/vat_rate.ts", "python/acme_pricing/deps/tax/rules/vat_rate.py", "go/deps/tax/rules/vatrate/vat_rate.go"] {
        assert_has(&out.join(f), "Source: @tax/rules/vat_rate.rule (rule vat_rate v1, sha256:");
        assert_has(&out.join(f), ", module example.com/acme/tax 1.0.0)\n");
    }
    assert_has(&out.join("python/acme_pricing/deps/__init__.py"), "from . import tax as tax\n");
    let (code, stdout, _) = generate(&t, "pricing", &["--embed", "tax", "--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
    // shop reaches tax through pricing, whose library holds it
    let (code, _, err) = generate(&t, "shop", &[]);
    assert_eq!(code, 0, "{err}");
    assert!(!files(&t.path().join("out/shop")).iter().any(|f| f.contains("deps/")));
}

/// What a library refuses, each with exit 2 and nothing written, said in English and in Japanese
/// (the golden file refused.txt): `--embed` without `--library`; paths, `--name` and `--module`
/// with it; `--embed` of a name the tree has not, of a dependency with a line, and of one a library
/// holds; a language with no line, no line at all, no version (the module's, a dependency's); a
/// Python name Python cannot import; a pre-release PEP 440 has no form of (the module's, a
/// dependency's); a directory that is not a module; and a contribution to a rule a library holds,
/// which the library's code cannot take.
#[test]
fn what_a_library_refuses() {
    type Setup = fn(&TempDir);
    let all: &[&str] = &["tax", "pricing", "shop"];
    let cases: &[(&str, &[&str], &[&str], Setup)] = &[
        ("pricing", &["gen", "--embed", "tax"], all, |_| {}),
        ("pricing", &["gen", "--library", "rules/shipping_fee.rule"], all, |_| {}),
        ("pricing", &["gen", "--library", "--name", "pricing"], all, |_| {}),
        ("pricing", &["gen", "--library", "--module", "example.com/acme/pricing-go"], all, |_| {}),
        ("pricing", &["gen", "--library", "--embed", "nosuch"], all, |_| {}),
        ("pricing", &["gen", "--library", "--embed", "tax"], all, |_| {}),
        ("shop", &["gen", "--library", "--embed", "tax"], &["pricing", "shop"], |_| {}),
        ("pricing", &["gen", "--library", "--target", "go"], all, |t| replace(t, "pricing/ritsu.mod", "library go \"example.com/acme/pricing-go\"\n", "")),
        ("pricing", &["gen", "--library"], &["tax"], |_| {}),
        ("pricing", &["gen", "--library"], all, |t| replace(t, "pricing/ritsu.mod", "version 1.1.0\n", "")),
        ("pricing", &["gen", "--library"], all, |t| replace(t, "tax/ritsu.mod", "version 1.0.0\n", "")),
        ("pricing", &["gen", "--library", "--target", "python"], all, |t| replace(t, "pricing/ritsu.mod", "\"acme-pricing\"", "\"2pricing\"")),
        ("pricing", &["gen", "--library", "--target", "python"], all, |t| replace(t, "pricing/ritsu.mod", "version 1.1.0", "version 1.2.0-dev.1")),
        ("pricing", &["gen", "--library", "--target", "python"], all, |t| replace(t, "tax/ritsu.mod", "version 1.0.0", "version 1.0.0-dev.1")),
        ("loose", &["gen", "--library"], all, |t| {
            t.write("loose/rules/shipping_fee.rule", t.read("pricing/rules/shipping_fee.rule"));
        }),
        ("shop", &["gen", "--library"], all, |t| {
            t.write("pricing/rules/order_discount.rule", ORDER_DISCOUNT);
            t.write("shop/rules/spring_sale.rule", SPRING_SALE);
        }),
    ];
    let mut said = String::new();
    for (dir, args, with, setup) in cases {
        let t = example(with);
        setup(&t);
        let out = t.path().join("out");
        said.push_str(&format!("── {dir}: ritsu {} (library lines: {})\n", args.join(" "), with.join(", ")));
        for lang in ["en", "ja"] {
            let mut args: Vec<&str> = args.to_vec();
            args.extend(["--out", out.to_str().unwrap(), "--lang", lang]);
            let (code, stdout, err) = ritsu_in(&t, dir, &args);
            assert_eq!((code, stdout.as_str()), (2, ""), "{dir} {args:?}: {err}");
            assert!(!out.exists(), "{dir} {args:?}: nothing is written");
            said.push_str(&err);
        }
        said.push('\n');
    }
    let mut failures = Vec::new();
    golden("tests/golden/library/refused.txt", &said, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// From version 2, the Go module's path ends in `/v<major>`, and the code that imports it imports it
/// so; npm and Python ask for it by the range up to the next major version.
#[test]
fn a_major_version_from_2_is_in_the_go_modules_path() {
    let t = example(&["tax", "pricing", "shop"]);
    replace(&t, "pricing/ritsu.mod", "version 1.1.0", "version 2.0.0");
    for m in ["pricing", "shop"] {
        let (code, _, err) = generate(&t, m, &[]);
        assert_eq!(code, 0, "{m}: {err}");
    }
    let out = t.path().join("out");
    assert_has(&out.join("pricing/go/go.mod"), "\nmodule example.com/acme/pricing-go/v2\n");
    assert_has(&out.join("pricing/go/doc.go"), "\npackage pricing_go\n");
    assert_has(&out.join("shop/go/go.mod"), "\texample.com/acme/pricing-go/v2 v2.0.0\n");
    assert_has(&out.join("shop/go/glue/basket/basket.go"), "pricingordertotal \"example.com/acme/pricing-go/v2/rules/ordertotal\"\n");
    assert_has(&out.join("shop/go/units/units.go"), "import pricinggounits \"example.com/acme/pricing-go/v2/units\"\n");
    assert_has(&out.join("shop/typescript/package.json"), "\"@acme/pricing\": \"^2.0.0\"");
    assert_has(&out.join("shop/python/pyproject.toml"), "\"acme-pricing>=2.0.0,<3.0.0\",\n");
}

/// The module's `source` is the npm package's `repository`, which npm's provenance checks against
/// the repository the package is built in, and the `Repository` of the PyPI project's URLs, which
/// PyPI marks verified when a trusted publisher publishes from it. The npm package's is written as
/// npm writes the URL of a repository on GitHub when it publishes (it would warn otherwise); the
/// PyPI project's, as the `source` line writes it. A module with no `source` has neither (the
/// golden files).
#[test]
fn the_modules_source_is_the_repository_of_the_npm_package_and_the_pypi_project() {
    let t = example(&["tax", "pricing", "shop"]);
    replace(&t, "pricing/ritsu.mod", "license \"MIT OR Apache-2.0\"\n", "license \"MIT OR Apache-2.0\"\nsource \"https://github.com/acme/pricing\"\n");
    let (code, _, err) = generate(&t, "pricing", &[]);
    assert_eq!(code, 0, "{err}");
    assert_has(
        &t.path().join("out/pricing/typescript/package.json"),
        "  \"license\": \"MIT OR Apache-2.0\",\n  \"repository\": {\n    \"type\": \"git\",\n    \"url\": \"git+https://github.com/acme/pricing.git\"\n  },\n",
    );
    assert_has(&t.path().join("out/pricing/python/pyproject.toml"), "dependencies = [\n]\n\n[project.urls]\nRepository = \"https://github.com/acme/pricing\"\n\n[build-system]\n");
}

/// A library's package.json says it is ritsu's in its `"//"` key, JSON having no comment, and ritsu
/// reads it so: where ritsu writes it no more, `--check` counts it as left and the next run takes
/// it out, as it does a file whose head says so. A package.json of someone else's stays.
#[test]
fn a_package_json_is_ritsus_by_its_slashes_key() {
    let t = example(&["tax", "pricing", "shop"]);
    let (code, _, err) = generate(&t, "pricing", &["--target", "typescript"]);
    assert_eq!(code, 0, "{err}");
    let ours = t.read("out/pricing/typescript/package.json");
    let key = format!("{{\n  \"//\": \"Code generated by ritsu {}. DO NOT EDIT.\",\n  \"name\": \"@acme/pricing\",\n", env!("CARGO_PKG_VERSION"));
    assert!(ours.starts_with(&key), "{ours}");
    // where an earlier run would have written it, and someone else's beside it
    t.write("out/pricing/typescript/old/package.json", &ours);
    t.write("out/pricing/typescript/theirs/package.json", "{\n  \"name\": \"theirs\",\n  \"private\": true\n}\n");
    let (code, stdout, _) = generate(&t, "pricing", &["--target", "typescript", "--check"]);
    assert_eq!((code, stdout.as_str()), (1, "generated no more, and still there: <dir>/out/pricing/typescript/old/package.json\n"));
    let (code, stdout, err) = generate(&t, "pricing", &["--target", "typescript"]);
    assert_eq!(code, 0, "{err}");
    assert!(stdout.contains("removed, as it is generated no more: <dir>/out/pricing/typescript/old/package.json\n"), "{stdout}");
    assert!(!t.path().join("out/pricing/typescript/old/package.json").exists());
    assert!(t.path().join("out/pricing/typescript/theirs/package.json").exists());
}

/// A rule and a gate read a dependency's files from its library, as the flow and the kaname module
/// of the example do: shop's rule takes an enum of an `enums` file of pricing's (rulec's import of
/// the file's module), and shop's gate asks a rule of pricing's (sekisho's import of the rule's
/// module). Neither of pricing's files comes into shop's library.
#[test]
fn a_rule_and_a_gate_read_a_dependencys_files_from_its_library() {
    let t = example(&["tax", "pricing", "shop"]);
    t.write("pricing/rules/sizes.rule", "enums sizes v1\ndescription \"The sizes of an order, and the ones a clerk refunds\"\n\nenum size = small | medium | large\ngroup refundable = small\n");
    t.write(
        "pricing/rules/refund.rule",
        "rule refund v1\ndescription \"Whether a clerk refunds an order: the sizes pricing calls refundable\"\n\nimport enum size from \"sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  allowed : bool\n\ntable allowed\npolicy unique\n| size            | -> allowed |\n| refundable      | true       |\n| not: refundable | false      |\n",
    );
    t.write(
        "shop/rules/packing.rule",
        "rule packing v1\ndescription \"The fee for packing an order, by its size\"\n\nimport enum size from \"@pricing/rules/sizes.rule\"\n\ninputs\n  size : size\n\noutputs\n  fee : money[GBPc]  round up(1GBPc)\n\ntable fee_by_size\npolicy unique\n| size   | -> fee : money[GBPc] |\n| small  | 100GBPc              |\n| medium | 200GBPc              |\n| large  | 300GBPc              |\n",
    );
    t.write(
        "shop/authz/refunds.gate",
        "gate refunds v1\ndescription \"A clerk refunds an order of a size the rule lets through\"\nnamespace Shop\n\nuse rule refund from \"@pricing/rules/refund.rule\"\n\nenum size = small | medium | large\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  description \"A member of the staff\"\n  roles clerk\n\nresource Order\n  description \"An order of the shop\"\n  attributes\n    size : size\n\naction refund_order\n  description \"Refund an order\"\n  principal User\n  resource Order\n  context\n    refundable = refund(size: resource.size).allowed\n\npermit clerks_refund\n  description \"A clerk refunds an order the rule lets through\"\n  principal in clerk\n  action refund_order\n  when refundable\n",
    );
    let (code, _, err) = generate(&t, "shop", &[]);
    assert_eq!(code, 0, "{err}");
    let out = t.path().join("out/shop");
    for (file, line) in [
        ("typescript/rules/packing.ts", "import { Size } from \"@acme/pricing/rules/sizes\";\n"),
        ("python/acme_shop/rules/packing.py", "from acme_pricing.rules.sizes import Size as Size\n"),
        ("go/rules/packing/packing.go", "\t\"example.com/acme/pricing-go/rules/sizes\"\n"),
        ("typescript/authz/refunds.ts", "import * as rule_pricing_refund from \"@acme/pricing/rules/refund\";\n"),
        ("python/acme_shop/authz/refunds.py", "from acme_pricing.rules import refund as _rules_pricing_refund\n"),
        ("go/authz/refunds/refunds.go", "\trulespricingrefund \"example.com/acme/pricing-go/rules/refund\"\n"),
    ] {
        assert_has(&out.join(file), line);
    }
    let theirs: Vec<String> = files(&out).into_iter().filter(|f| f.contains("deps/") || f.contains("rules/sizes") || f.contains("rules/refund")).collect();
    assert!(theirs.is_empty(), "shop's library holds files of pricing's: {theirs:?}");
}

/// What the registries' tools write beside a library at its root (tsc's dist/, setuptools' build/
/// and its egg-info, the go.sum of a module) is not ritsu's: `--check` does not count it, and the
/// next run leaves it, though tsc and setuptools copy the head that says ritsu made a file.
#[test]
fn what_the_registries_tools_write_is_left_alone() {
    let t = example(&["tax", "pricing", "shop"]);
    let (code, _, err) = generate(&t, "pricing", &[]);
    assert_eq!(code, 0, "{err}");
    let head = format!("// Code generated by ritsu {}. DO NOT EDIT.\n", env!("CARGO_PKG_VERSION"));
    let theirs = [
        ("out/pricing/typescript/dist/index.js", head.clone()),
        ("out/pricing/typescript/dist/rules/order_total.d.ts", head.clone()),
        ("out/pricing/python/build/lib/acme_pricing/__init__.py", head.replace("//", "#")),
        ("out/pricing/python/acme_pricing.egg-info/PKG-INFO", "Metadata-Version: 2.4\n".to_string()),
        ("out/pricing/go/go.sum", "example.com/acme/tax-go v1.0.0/go.mod h1:0000\n".to_string()),
    ];
    for (rel, text) in &theirs {
        t.write(rel, text);
    }
    let (code, stdout, _) = generate(&t, "pricing", &["--check"]);
    assert_eq!((code, stdout.as_str()), (0, ""));
    let (code, _, err) = generate(&t, "pricing", &[]);
    assert_eq!(code, 0, "{err}");
    for (rel, text) in &theirs {
        assert_eq!(&t.read(rel), text, "{rel} is left as it was");
    }
}

/// An open rule of `pricing`'s, which the campaigns of the modules that use it add to (as in
/// tests/modules_gen.rs).
const ORDER_DISCOUNT: &str = "# The discount of an order: what the campaigns of the shop add, up to its subtotal.\nrule order_discount v1\ndescription \"The discount of an order: the sum of what the campaigns add, up to the subtotal\"\n\ninputs\n  subtotal : money[GBPc]  range >=0GBPc <=2_000_000GBPc\n\noutputs\n  discount : money[GBPc]  round down(1GBPc)\n\n# What the campaigns take off, from files of their own: nothing when none does.\nopen campaigns : money[GBPc]  combine sum  empty 0GBPc  range >=0GBPc <=10_000GBPc\n\ndefine discount : money[GBPc] = min(campaigns, subtotal)\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 0GBPc       |\n";

/// `shop`'s campaign, added to `pricing`'s `order_discount`.
const SPRING_SALE: &str = "# A campaign of the shop, added to the rule order_discount of the module pricing.\ncontribution spring_sale v1\ndescription \"A spring sale: 100 pence off every order\"\nto \"@pricing/rules/order_discount.rule\" signature:67a223ec46dce359\n\ndefine sale_off : money[GBPc] = 100GBPc\n\ninto campaigns\n  sale_off\n\nexamples\n| subtotal | -> discount |\n| 1000GBPc | 100GBPc     |\n| 50GBPc   | 50GBPc      |\n";

// ── the pages ────────────────────────────────────────────────────────────────────────────────────

/// The pages on publishing libraries (website/docs/libraries.md and the Japanese one) show what
/// ritsu writes and says, each in its language. `pricing`'s ritsu.mod is the block under
/// `<!-- pricing/ritsu.mod -->`, and `tax` and `shop` have their lines. A `console` block under
/// `<!-- run in pricing -->` is run in `pricing` and prints what it shows (a line `…` for lines
/// left out); the one under `<!-- run in pricing, with no library lines in tax's ritsu.mod -->` is
/// run with `tax`'s lines left out. A block under `<!-- out/<module>/<path> -->` is lines of that
/// file of the module's libraries, in order. The two pages mark the same blocks.
#[test]
fn the_pages_show_what_ritsu_writes_and_says() {
    let mut marks: Vec<Vec<String>> = Vec::new();
    for (name, lang) in [("website/docs/libraries.md", "en"), ("website/docs-ja/libraries.md", "ja")] {
        let p = common::page(name);
        let lines: Vec<&str> = p.text.lines().collect();
        let marked: Vec<(&str, &common::Block)> = p.blocks.iter().filter_map(|b| Some((lines.get(b.at.checked_sub(2)?)?.trim().strip_prefix("<!-- ")?.strip_suffix(" -->")?, b))).collect();
        let ritsu_mod = marked.iter().find(|(m, _)| *m == "pricing/ritsu.mod").map(|(_, b)| format!("{}\n", b.lines.join("\n")));
        let ritsu_mod = ritsu_mod.unwrap_or_else(|| panic!("{name} shows no pricing/ritsu.mod"));
        let full = example(&["tax", "shop"]);
        let bare = example(&["shop"]);
        for t in [&full, &bare] {
            t.write("pricing/ritsu.mod", &ritsu_mod);
        }
        let mut wrong = Vec::new();
        // the commands first: they write pricing's libraries
        for (m, b) in &marked {
            let t = match *m {
                "run in pricing" => &full,
                "run in pricing, with no library lines in tax's ritsu.mod" => &bare,
                _ => continue,
            };
            for (cmd, out) in common::runs(b) {
                let words: Vec<&str> = cmd.split_whitespace().collect();
                assert_eq!(words.first(), Some(&"ritsu"), "{name}:{}: `{cmd}` is not ritsu's", b.at);
                let (_, stdout, stderr) = ritsu_in(t, "pricing", &words[1..]);
                if !common::same(&common::lines_of(&out.join("\n")), &common::lines_of(&format!("{stdout}{stderr}"))) {
                    wrong.push(format!("{name}:{}: `{cmd}` prints\n{stdout}{stderr}", b.at));
                }
            }
        }
        for m in ["tax", "shop"] {
            let (code, _, err) = generate(&full, m, &["--lang", lang]);
            assert_eq!(code, 0, "{m}: {err}");
        }
        for (m, b) in marked.iter().filter(|(m, _)| m.starts_with("out/")) {
            if !common::lines_in_order(&common::shown_lines(b), &common::trimmed(&read(&full.path().join(m)))) {
                wrong.push(format!("{name}:{}: the block is not lines of {m}, in order", b.at));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        marks.push(marked.iter().map(|(m, _)| m.to_string()).collect());
    }
    assert!(marks[0].len() >= 15, "{} blocks marked on the English page: were the marks changed?", marks[0].len());
    assert_eq!(marks[0], marks[1], "the two pages mark the same blocks");
}

// ── built, installed and used ────────────────────────────────────────────────────────────────────

/// The three libraries built, installed and used by a host in each language (the module's doc
/// says how). The host prices the basket of kaname's example in `shop` (two teas at 450 pence and
/// a book at 1200: 2100 pence, and 2915 with the standard VAT and 395 pence of shipping), with
/// `shop`'s subtotal given to `pricing`'s rule as it is.
#[test]
fn the_libraries_are_built_and_a_host_uses_them() {
    let t = example(&["tax", "pricing", "shop"]);
    for m in ["tax", "pricing", "shop"] {
        let (code, _, err) = generate(&t, m, &[]);
        assert_eq!(code, 0, "{m}: {err}");
    }
    if ritsu_testkit::need(Need::Node) {
        match tsc() {
            Some((tsc, modules)) => {
                typescript(&t, &tsc, &modules);
                println!("tsc builds the three TypeScript libraries, and the host passes tsc --strict and runs on Node");
            }
            None => ritsu_testkit::skip("TypeScript (crates/dandori/tools/temporal) or tigerbeetle-node (crates/chobo/tools/runner) is not installed; the TypeScript libraries are not built"),
        }
    }
    if ritsu_testkit::need(Need::Python) {
        let builder = workspace().join("crates/dandori/tools/wire/.venv/bin/python");
        match mypy() {
            Some((mypy, python, _)) if builder.exists() => {
                python_libraries(&t, &mypy, &python, &builder);
                println!("setuptools builds the three Python libraries, and the host passes mypy --strict and runs");
            }
            _ => ritsu_testkit::skip(
                "mypy, Temporal's Python SDK (crates/dandori/tools/temporal-python), tigerbeetle (crates/chobo/tools/runner) or setuptools (crates/dandori/tools/wire) is not installed; the Python libraries are not built",
            ),
        }
    }
    if ready(Need::Go, have_go, "go is not installed; the Go libraries are not built") {
        match go_module() {
            Some(gomod) => {
                go_libraries(&t, &gomod);
                println!("the host's module builds the three Go libraries, and passes go vet and runs");
            }
            None => ritsu_testkit::skip("the go.mod of crates/dandori/tools/temporal-go or crates/chobo/tools/runner/go is missing; the Go libraries are not built"),
        }
    }
}

/// The output of a command that must succeed, said with `what` when it does not.
fn run(c: &mut Command, what: &str) -> String {
    let o = c.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(o.status.success(), "{what}:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Each TypeScript library built as `npm install` builds it (its `prepare`, tsc with the library's
/// tsconfig.json), in the order they depend on each other, with Temporal's SDK linked into its
/// node_modules and each library it reads copied there as npm installs a registry's package (npm's
/// `file:` links instead, and from the link's target the declarations of `pricing` do not find
/// `tax`). Then the host, held to `tsc --strict` with the libraries' declarations checked too, and
/// run on Node.
fn typescript(t: &TempDir, tsc: &Path, modules: &[PathBuf]) {
    let ts = t.path().join("ts");
    let install = |name: &str, into: &Path| {
        let at = into.join("node_modules/@acme").join(name);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::copy(ts.join(name).join("package.json"), at.join("package.json")).unwrap();
        ritsu_testkit::tmp::copy_dir(&ts.join(name).join("dist"), &at.join("dist"));
    };
    for (m, reads) in [("tax", &[][..]), ("pricing", &["tax"][..]), ("shop", &["pricing", "tax"][..])] {
        let dir = ts.join(m);
        ritsu_testkit::tmp::copy_dir(&t.path().join("out").join(m).join("typescript"), &dir);
        let nm = dir.join("node_modules");
        std::fs::create_dir_all(&nm).unwrap();
        for p in modules {
            std::os::unix::fs::symlink(p, nm.join(p.file_name().unwrap())).unwrap();
        }
        for r in reads {
            install(r, &dir);
        }
        run(Command::new(tsc).args(["-p", "tsconfig.json"]).current_dir(&dir), &format!("tsc builds {m} with faults"));
        assert!(dir.join("dist/index.d.ts").exists() && dir.join("dist/index.js").exists(), "{m}: tsc wrote no dist/index");
    }
    let host = ts.join("host");
    std::fs::create_dir_all(&host).unwrap();
    for m in ["tax", "pricing", "shop"] {
        install(m, &host);
    }
    std::fs::write(
        host.join("tsconfig.json"),
        "{ \"compilerOptions\": { \"strict\": true, \"module\": \"nodenext\", \"moduleResolution\": \"nodenext\", \"target\": \"es2022\", \"outDir\": \"out\" }, \"include\": [\"main.ts\"] }\n",
    )
    .unwrap();
    std::fs::write(host.join("main.ts"), HOST_TS).unwrap();
    run(Command::new(tsc).args(["-p", "tsconfig.json"]).current_dir(&host), "tsc --strict finds fault with the host");
    let said = run(Command::new("node").arg("out/main.js").current_dir(&host), "the host fails on Node");
    assert_eq!(said, "subtotal 2100, total 2915\n");
}

const HOST_TS: &str = r#"import { order_total, Category } from "@acme/pricing/rules/order_total";
import type { GBPc, Gram } from "@acme/pricing/units";
import { subtotal, total } from "@acme/shop/glue/basket";
import type { Line } from "@acme/shop/glue/basket";
import type * as shop from "@acme/shop/units";

const gbp = (n: bigint): GBPc => n as GBPc;
const lines: Line[] = [
  { sku: "tea", qty: 2n, unit_price: gbp(450n) },
  { sku: "book", qty: 1n, unit_price: gbp(1200n) },
];
// the shop's amount of money is the pricing library's: one type
const sub: shop.GBPc = subtotal(lines);
const byRule: GBPc = order_total(sub, Category.STANDARD, gbp(395n));
const byShop: GBPc = total(lines, Category.STANDARD, gbp(395n));
// @ts-expect-error a mass is not an amount of money
const wrong: GBPc = 5n as Gram;
void wrong;
if (sub !== 2100n || byRule !== 2915n || byShop !== 2915n) {
  throw new Error(`unexpected: ${sub} ${byRule} ${byShop}`);
}
console.log(`subtotal ${sub}, total ${byShop}`);
"#;

/// Where a Python puts the packages installed into it.
fn purelib(python: &Path) -> PathBuf {
    PathBuf::from(run(Command::new(python).args(["-c", "import sysconfig; print(sysconfig.get_paths()['purelib'])"]), "the Python says no purelib").trim())
}

/// Each Python library built into a wheel by setuptools, the build backend its pyproject.toml
/// names (dandori's tools/wire has it); the three unpacked into a new environment, as an installer
/// puts a wheel's files, with Temporal's SDK beside them by a .pth (dandori's
/// tools/temporal-python); each library's code held to `mypy --strict` with that environment's
/// Python, which reads the libraries it imports as installed packages (their py.typed; mypy says
/// nothing of the code of an installed package, so each is checked from its own directory); and
/// the host held to it too, and run.
fn python_libraries(t: &TempDir, mypy: &Path, python: &Path, builder: &Path) {
    let py = t.path().join("py");
    let wheels = py.join("wheels");
    std::fs::create_dir_all(&wheels).unwrap();
    for m in ["tax", "pricing", "shop"] {
        let dir = py.join(m);
        ritsu_testkit::tmp::copy_dir(&t.path().join("out").join(m).join("python"), &dir);
        let build = "import sys\nfrom setuptools import build_meta\nbuild_meta.build_wheel(sys.argv[1])\n";
        run(Command::new(builder).args(["-c", build]).arg(&wheels).current_dir(&dir), &format!("setuptools builds no wheel of {m}"));
    }
    let mut built: Vec<String> = std::fs::read_dir(&wheels).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    built.sort();
    assert_eq!(built, ["acme_pricing-1.1.0-py3-none-any.whl", "acme_shop-0.1.0-py3-none-any.whl", "acme_tax-1.0.0-py3-none-any.whl"]);
    let env = py.join("env");
    run(Command::new(python).args(["-m", "venv", "--without-pip"]).arg(&env), "python -m venv");
    let env_python = env.join("bin/python");
    let site = purelib(&env_python);
    for w in &built {
        run(Command::new(&env_python).args(["-m", "zipfile", "-e"]).arg(wheels.join(w)).arg(&site), &format!("{w} does not unpack"));
    }
    let shop = read(&site.join("acme_shop-0.1.0.dist-info/METADATA"));
    assert!(shop.contains("Requires-Dist: acme-pricing<2.0.0,>=1.1.0\n") || shop.contains("Requires-Dist: acme-pricing>=1.1.0,<2.0.0\n"), "{shop}");
    assert!(read(&site.join("acme_pricing-1.1.0.dist-info/METADATA")).contains("License-Expression: MIT OR Apache-2.0\n"));
    std::fs::write(site.join("temporal.pth"), format!("{}\n", purelib(python).display())).unwrap();
    let strict = |dir: &Path, what: &[&str], fault: &str| {
        run(Command::new(mypy).args(["--strict", "--no-incremental", "--cache-dir"]).arg(py.join(".mypy_cache")).arg("--python-executable").arg(&env_python).args(what).current_dir(dir), fault);
    };
    for m in ["tax", "pricing", "shop"] {
        strict(&py.join(m), &["-p", &format!("acme_{m}")], &format!("mypy --strict finds fault with the library acme-{m}"));
    }
    let host = py.join("host");
    std::fs::create_dir_all(&host).unwrap();
    std::fs::write(host.join("main.py"), HOST_PY).unwrap();
    strict(&host, &["main.py"], "mypy --strict finds fault with the host");
    let said = run(Command::new(&env_python).arg("main.py").current_dir(&host), "the host fails");
    assert_eq!(said, "subtotal 2100, total 2915\n");
}

const HOST_PY: &str = r#"from acme_pricing.rules.order_total import Category, order_total
from acme_pricing.units import GBPc, Gram
from acme_shop import units as shop
from acme_shop.glue.basket import Line, subtotal, total

lines = [Line(sku="tea", qty=2, unit_price=GBPc(450)), Line(sku="book", qty=1, unit_price=GBPc(1200))]
# the shop's amount of money is the pricing library's: one type
sub: shop.GBPc = subtotal(lines=lines)
by_rule: GBPc = order_total(sub, Category.STANDARD, GBPc(395))
by_shop: GBPc = total(lines=lines, category=Category.STANDARD, shipping=GBPc(395))
wrong: GBPc = Gram(5)  # type: ignore[assignment]
assert (sub, by_rule, by_shop) == (2100, 2915, 2915), (sub, by_rule, by_shop)
print(f"subtotal {sub}, total {by_shop}")
"#;

/// The three Go libraries as the modules they are, required by the host's module by their paths
/// and versions and put at their directories by `replace`, as the module proxy hands them out
/// once tagged; the host's go.mod and go.sum otherwise those of the tools' modules
/// ([`go_module`]), so that it builds with no network. Every package of the three vetted from the
/// host (as a library is vetted before it is tagged) and left as gofmt writes it; the host vetted
/// and run; then a mass given where `pricing` takes money, which the compiler refuses.
fn go_libraries(t: &TempDir, gomod: &(String, String)) {
    let go = t.path().join("go");
    for m in ["tax", "pricing", "shop"] {
        ritsu_testkit::tmp::copy_dir(&t.path().join("out").join(m).join("go"), &go.join(m));
    }
    let host = go.join("host");
    std::fs::create_dir_all(&host).unwrap();
    let mut module = gomod.0.replacen("module gocheck\n", "module example.com/host\n", 1);
    module.push_str("\nrequire (\n\texample.com/acme/pricing-go v1.1.0\n\texample.com/acme/shop-go v0.1.0\n)\n");
    module.push_str("\nreplace (\n\texample.com/acme/pricing-go => ../pricing\n\texample.com/acme/shop-go => ../shop\n\texample.com/acme/tax-go => ../tax\n)\n");
    std::fs::write(host.join("go.mod"), module).unwrap();
    std::fs::write(host.join("go.sum"), &gomod.1).unwrap();
    std::fs::write(host.join("main.go"), HOST_GO).unwrap();
    run(go_in(&host).args(["vet", "example.com/acme/tax-go/...", "example.com/acme/pricing-go/...", "example.com/acme/shop-go/..."]), "go vet finds fault with the libraries");
    let unformatted = run(Command::new("gofmt").arg("-l").args(["tax", "pricing", "shop"]).current_dir(&go), "gofmt");
    assert_eq!(unformatted, "", "gofmt would write these otherwise");
    run(go_in(&host).args(["vet", "./..."]), "go vet finds fault with the host");
    let said = run(go_in(&host).args(["run", "."]), "the host fails");
    assert_eq!(said, "subtotal 2100, total 2915\n");
    std::fs::create_dir_all(host.join("wrong")).unwrap();
    std::fs::write(
        host.join("wrong/wrong.go"),
        "package wrong\n\nimport \"example.com/acme/pricing-go/units\"\n\n// Shipping is a mass where money is taken.\nvar Shipping units.GBPc = units.Gram(5)\n",
    )
    .unwrap();
    let o = go_in(&host).args(["build", "./wrong"]).output().unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!o.status.success() && err.contains("cannot use units.Gram(5)"), "a mass is taken where money is:\n{err}");
}

const HOST_GO: &str = r#"package main

import (
	"fmt"

	"example.com/acme/pricing-go/rules/ordertotal"
	"example.com/acme/pricing-go/units"
	"example.com/acme/shop-go/glue/basket"
	shopunits "example.com/acme/shop-go/units"
)

func main() {
	lines := []basket.Line{{Sku: "tea", Qty: 2, UnitPrice: units.GBPc(450)}, {Sku: "book", Qty: 1, UnitPrice: units.GBPc(1200)}}
	// the shop's amount of money is the pricing library's: one type
	var sub shopunits.GBPc = basket.Subtotal(lines)
	byRule, err := ordertotal.OrderTotal(ordertotal.Input{Subtotal: sub, Category: ordertotal.CategoryStandard, Shipping: units.GBPc(395)})
	if err != nil {
		panic(err)
	}
	byShop := basket.Total(lines, ordertotal.CategoryStandard, units.GBPc(395))
	if sub != 2100 || byRule != 2915 || byShop != 2915 {
		panic(fmt.Sprint("unexpected: ", sub, byRule, byShop))
	}
	fmt.Printf("subtotal %d, total %d\n", sub, byShop)
}
"#;
