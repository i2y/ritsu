//! A rule may name an input, a definition, a table output or a sequence anything — `int`, `float`,
//! `bool`, `repr`, `len`, `str`, `list` — and the functions generated from it still work (§15.207).
//!
//! Python's builtins are names like any other, and a function that has a parameter called `len`
//! and says `len(items)` calls the parameter. So a function that holds the rule's names does not
//! call a builtin by its own name: the module binds `_int = int`, `_len = len` and the like above
//! the functions, and the functions call those. It also gives the comment over those bindings
//! the blank lines it did not have to be cut by.
//!
//! The materials are rules whose names are the builtins, English first with the Japanese version
//! of each beside it (`tests/builtins/`): a fee decided from inputs, table outputs and a walk
//! over a sequence; a fee read out of an order object whose shape is called by the name of a
//! builtin the projection function calls; and groups called by the names the module gives to the
//! builtins and to its helpers. The generated code of those and of the rules of the corpus that
//! walk, fold, count, carry a machine, take optional inputs or read a caller's object is looked
//! through for a builtin called by its bare name.

use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd).arg(arg).output().map(|o| o.status.success()).unwrap_or(false)
}

/// What one language of the materials is called and says.
struct Case {
    lang: &'static str,
    /// The two rules of `tests/builtins/`: the fee, and the fee read out of an order.
    rules: [&'static str; 2],
    /// The modules they are generated as.
    fee: &'static str,
    order: &'static str,
    /// What the fee refuses: an input that is not an integer, a truth value that is not one, a
    /// value that is not a member of the enum, a number out of range, an element that is not a
    /// truth value, and a sequence that is too long.
    not_integer: &'static str,
    not_bool: &'static str,
    not_in_enum: &'static str,
    out_of_range: &'static str,
    element_not_bool: &'static str,
    too_many: &'static str,
}

const CASES: [Case; 2] = [
    Case {
        lang: "en",
        rules: ["tests/builtins/builtin_names.rule", "tests/builtins/builtin_names_shape.rule"],
        fee: "builtin_names",
        order: "builtin_names_shape",
        not_integer: "int is not an integer",
        not_bool: "bool is not a boolean",
        not_in_enum: "len is not a value of enum size",
        out_of_range: "repr is out of range",
        element_not_bool: "set is not a boolean",
        too_many: "str has too many elements (at most 50)",
    },
    Case {
        lang: "ja",
        rules: ["tests/builtins/組み込み関数と同じ名前.rule", "tests/builtins/組み込み関数と同じ名前の注文.rule"],
        fee: "builtin_names_ja",
        order: "builtin_names_shape_ja",
        not_integer: "整数 が整数ではありません",
        not_bool: "真偽 が真偽ではありません",
        not_in_enum: "長さ が列挙 大きさ区分 の値ではありません",
        out_of_range: "表記 が範囲の外です",
        element_not_bool: "条件甲 が真偽ではありません",
        too_many: "明細 の要素が多すぎます（上限 50）",
    },
];

/// The rules whose functions are looked through, English first and the Japanese version beside
/// it: the two above, and the rules of the corpus that walk a sequence and count it
/// (`supplier_matching`, `納入先照合`), fold it (`nationwide_freight`, `全国運賃`), sum it
/// (`cart_shipping_fee`, `買物かごの送料`), carry a machine (`order_lifecycle`, `注文の状態`),
/// take optional inputs (`parcel_cover`, `小包の補償`), and read a caller's object through a JSON
/// Schema (`order_shipping_fee`, `注文の送料`) and through a `.proto` (`express_delivery_quote`,
/// `速達の見積`, and `shipment_delivery_fee`, `出荷の送料`, which also asks its lists).
const LOOKED_AT: [[&str; 2]; 10] = [
    ["tests/builtins/builtin_names.rule", "tests/builtins/組み込み関数と同じ名前.rule"],
    ["tests/builtins/builtin_names_shape.rule", "tests/builtins/組み込み関数と同じ名前の注文.rule"],
    ["tests/corpus/supplier_matching.rule", "tests/corpus/納入先照合.rule"],
    ["tests/corpus/nationwide_freight.rule", "tests/corpus/全国運賃.rule"],
    ["tests/corpus/cart_shipping_fee.rule", "tests/corpus/買物かごの送料.rule"],
    ["tests/corpus/order_lifecycle.rule", "tests/corpus/注文の状態.rule"],
    ["tests/optional/parcel_cover.rule", "tests/optional/小包の補償.rule"],
    ["tests/corpus/order_shipping_fee.rule", "tests/corpus/注文の送料.rule"],
    ["tests/corpus/express_delivery_quote.rule", "tests/corpus/速達の見積.rule"],
    ["tests/corpus/shipment_delivery_fee.rule", "tests/corpus/出荷の送料.rule"],
];

/// `rulec gen` of some rules into a directory of their own.
fn generate(lang: &str, rules: &[&str]) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("builtin-names-{lang}"));
    let out = t.path().join("out");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", lang)
        .current_dir(root())
        .arg("gen")
        .args(rules)
        .arg("--out")
        .arg(&out)
        .output()
        .expect("cannot start rulec");
    assert!(o.status.success(), "{lang}: {}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    (t, out)
}

fn python(dir: &Path, script: &str, args: &[&str]) -> (bool, String) {
    let o = Command::new("python3")
        .current_dir(dir)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .args(["-B", "-X", "utf8", "-c", script])
        .args(args)
        .output()
        .expect("cannot start python3");
    (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join("python").join(file)).unwrap_or_else(|e| panic!("cannot read python/{file}: {e}"))
}

/// Every name a function of the rule — the one its alias names, its traced twin, the record
/// function and the projection function — reads from outside itself, and which the module does
/// not define: a builtin function called by its bare name, or a name nothing binds. (A generator
/// expression inside a function is looked through too.) The helpers of the module have none of the
/// rule's names in scope and are not looked at.
const LOOK: &str = r#"
import builtins, os, sys, symtable

# The builtin functions (the page of the Python documentation by that name is the lower-case ones);
# the exceptions, such as the AssertionError of a branch that cannot be reached, are not among them.
builtin = {n for n in dir(builtins) if n[0].islower()}
everything = set(dir(builtins))
out, seen = [], 0
for f in sorted(os.listdir(sys.argv[1])):
    if not f.endswith(".py") or f.startswith("_") or f.endswith(("_runner.py", "_mcp.py", "_service.py", "_connect_runner.py")):
        continue
    stem = f[:-3]
    table = symtable.symtable(open(os.path.join(sys.argv[1], f), encoding="utf-8").read(), f, "exec")
    defined = {s.get_name() for s in table.get_symbols() if s.is_assigned() or s.is_imported() or s.is_namespace()}

    def look(t, fn):
        for s in t.get_symbols():
            n = s.get_name()
            if s.is_global() and n not in defined and (n in builtin or n not in everything):
                out.append(f"{f}: {fn} reads {n}" + (", a builtin" if n in builtin else ", which nothing binds"))
        for c in t.get_children():
            look(c, fn)

    mine = {stem, stem + "_traced", stem + "_record", stem + "_from"}
    found = {c.get_name() for c in table.get_children() if c.get_name() in mine}
    for c in table.get_children():
        if c.get_name() in mine:
            look(c, c.get_name())
    if not {stem, stem + "_traced", stem + "_record"} <= found:
        out.append(f"{f}: the functions of the rule were not found: {sorted(found)}")
    seen += len(found)
print("\n".join(out) if out else f"none of {seen} functions")
sys.exit(1 if out else 0)
"#;

/// No function that holds the rule's names reads a builtin by its bare name, in the generated
/// code of rules that take every way there is to read an input, walk a sequence, fold, count and
/// read a caller's object.
#[test]
fn no_function_of_a_rule_calls_a_builtin_by_its_bare_name() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here; the functions are not looked through") {
        return;
    }
    for (k, lang) in ["en", "ja"].into_iter().enumerate() {
        let rules: Vec<&str> = LOOKED_AT.iter().map(|pair| pair[k]).collect();
        let (_t, out) = generate(lang, &rules);
        let (ok, said) = python(&out, LOOK, &[&out.join("python").to_string_lossy()]);
        assert!(ok, "{lang}: a function calls a builtin by its bare name:\n{said}");
        println!("builtin names: {lang}: {}", said.trim());
    }
}

/// A rule whose inputs, table outputs, output and sequence are named like builtins answers
/// what the reference evaluator answers (the runner on the vectors), refuses what it refuses with
/// the sentences it says, and reads the order object it is handed.
#[test]
fn a_rule_named_like_the_builtins_is_answered_and_refused_as_any_other() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here; the modules are not called") {
        return;
    }
    for c in &CASES {
        let (_t, out) = generate(c.lang, &c.rules);
        // The runner, on every vector of the fee: the answers of the reference evaluator.
        for (module, vectors) in [(c.fee, "expected"), (c.order, "expected")] {
            let input = std::fs::File::open(out.join("vectors").join(format!("{module}.jsonl"))).unwrap();
            let o = Command::new("python3")
                .current_dir(out.join("python"))
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .args(["-B", &format!("{module}_runner.py")])
                .stdin(input)
                .output()
                .unwrap();
            assert!(o.status.success(), "{}: {module}: {}", c.lang, String::from_utf8_lossy(&o.stderr));
            let want = std::fs::read_to_string(out.join("vectors").join(format!("{module}.{vectors}.jsonl"))).unwrap();
            let got = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(!want.is_empty() && got == want, "{}: {module}: the runner does not answer what the vectors expect", c.lang);
            // And the inputs the door refuses are refused, each with a line of its own.
            let refused = std::fs::File::open(out.join("vectors").join(format!("{module}.refused.jsonl"))).unwrap();
            let o = Command::new("python3")
                .current_dir(out.join("python"))
                .env("PYTHONDONTWRITEBYTECODE", "1")
                .args(["-B", &format!("{module}_runner.py")])
                .stdin(refused)
                .output()
                .unwrap();
            let said = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(o.status.success() && !said.is_empty() && said.lines().all(|l| l.starts_with("{\"refused\":\"input\"")), "{}: {module}: {said}", c.lang);
        }
        // Called as a caller calls it.
        let script = format!(
            "import {fee} as m, {order} as s\n\
             S, B, E = m.Size.SMALL, m.Size.BIG, m.Element\n\
             def refused(*a):\n\
             \x20   try:\n\
             \x20       m.{fee}(*a)\n\
             \x20   except m.RuleInputError as e:\n\
             \x20       return str(e)\n\
             \x20   raise SystemExit('answered: ' + repr(a))\n\
             assert m.{fee}(10, 5, 0, True, S, 0, []) == 150\n\
             assert m.{fee}(10.0, 5.0, 0.0, True, S, 0.0, []) == 150\n\
             assert m.{fee}(1, 5, 60, False, B, 100, [E(True, True), E(True, False)]) == 530\n\
             assert refused(10.5, 5, 0, True, S, 0, []) == {not_integer:?} + ': 10.5'\n\
             assert refused(10, 5, 0, 'yes', S, 0, []) == {not_bool:?} + \": 'yes'\"\n\
             assert refused(10, 5, 0, True, 'small', 0, []) == {not_in_enum:?} + \": 'small'\"\n\
             assert refused(10, 5, 200, True, S, 0, []) == {out_of_range:?} + ': 200'\n\
             assert refused(10, 5, 0, True, S, 0, [E(1, True)]) == {element_not_bool:?} + ': 1'\n\
             assert refused(10, 5, 0, True, S, 0, [E(True, True)] * 60) == {too_many:?} + ': 60'\n\
             order = {{'shipping': {{'zone': 'okinawa'}}, 'lines': [{{'sku': 'a', 'chilled': True, 'amount_jpy': 1}}, {{'sku': 'b', 'chilled': False, 'amount_jpy': 2}}]}}\n\
             assert s.{order}_from(order) == 2030\n\
             print('builtin names ok')\n",
            fee = c.fee,
            order = c.order,
            not_integer = c.not_integer,
            not_bool = c.not_bool,
            not_in_enum = c.not_in_enum,
            out_of_range = c.out_of_range,
            element_not_bool = c.element_not_bool,
            too_many = c.too_many,
        );
        let (ok, said) = python(&out.join("python"), &script, &[]);
        assert!(ok && said.trim() == "builtin names ok", "{}:\n{said}", c.lang);
    }
}

/// A group called `int`, or `json_str`, is held in a constant that does not take the name the
/// module gives the builtin `int` or its helper `_json_str`: it would be read as the builtin, or
/// replaced by it, and `len in _int` would not be a question about the group.
#[test]
fn a_group_called_like_a_bound_name_is_a_constant_of_its_own() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here; the modules are not called") {
        return;
    }
    // (the module, and the sentence for a value that is not a member of the enum)
    for (lang, rule, module, not_in_enum) in [
        ("en", "tests/builtins/group_names.rule", "group_names", "len is not a value of enum size"),
        ("ja", "tests/builtins/集合の名前.rule", "group_names_ja", "大きさ が列挙 大きさ区分 の値ではありません"),
    ] {
        let (_t, out) = generate(lang, &[rule]);
        let m = read(&out, &format!("{module}.py"));
        // The constants: one underscore more than the name, where the name is the module's own.
        assert!(m.contains("_int_ = frozenset(") && m.contains("_json_str_ = frozenset("), "{lang}: {m}");
        assert!(!m.contains("\n_int = frozenset(") && !m.contains("\n_json_str = frozenset("), "{lang}");
        let script = format!(
            "import {module} as m\n\
             S = m.Size\n\
             assert [(r.float, r.isinstance) for r in map(m.{module}, [S.SMALL, S.BIG, S.HUGE])] == [(False, True), (True, False), (True, True)]\n\
             try:\n\
             \x20   m.{module}('x')\n\
             \x20   raise SystemExit('answered')\n\
             except m.RuleInputError as e:\n\
             \x20   assert str(e) == {not_in_enum:?} + \": 'x'\", str(e)\n\
             print('groups ok')\n"
        );
        let (ok, said) = python(&out.join("python"), &script, &[]);
        assert!(ok && said.trim() == "groups ok", "{lang}:\n{said}");
    }
}

/// The comment over the names bound to the builtins is not cut into lines by blank lines, and
/// the names are bound one under another: the module reads
///
/// ```text
/// _Trace = list[Fired]
///
/// # …the comment…
/// _isinstance = isinstance
/// _int = int
/// ```
#[test]
fn the_comment_over_the_bound_names_stays_in_one_piece() {
    for c in &CASES {
        let (_t, out) = generate(c.lang, &c.rules);
        let m = read(&out, &format!("{}.py", c.fee));
        let lines: Vec<&str> = m.lines().collect();
        let at = lines.iter().position(|l| *l == "_isinstance = isinstance").unwrap_or_else(|| panic!("{}: no binding of isinstance", c.lang));
        // The comment: lines of its own right over the first binding, set off from what is above.
        let first = (0..at).rev().take_while(|k| lines[*k].starts_with("# ")).last().unwrap_or(at);
        assert!(at - first >= 3, "{}: the comment is {} lines, over\n{}", c.lang, at - first, lines[first.saturating_sub(2)..=at].join("\n"));
        assert!(lines[first - 1].is_empty() && !lines[first - 2].is_empty(), "{}: the comment is not set off by one blank line", c.lang);
        // The bindings: one under another, to the first blank line.
        let bound: Vec<&str> = lines[at..].iter().take_while(|l| !l.is_empty()).copied().collect();
        assert!(bound.len() >= 6, "{}: {bound:?}", c.lang);
        assert!(bound.iter().all(|l| l.split_once(" = ").is_some_and(|(a, b)| a.starts_with('_') && a[1..] == *b)), "{}: {bound:?}", c.lang);
        // Two blank lines, and the first helper.
        assert_eq!(&lines[at + bound.len()..at + bound.len() + 3], ["", "", "def _min(a: int, b: int) -> int:"], "{}", c.lang);
        // The head of the file is as it was: its comment lines stay together.
        assert!(lines[0].starts_with("# ") && lines[1].starts_with("# ") && lines[2].is_empty(), "{}", c.lang);
    }
}
