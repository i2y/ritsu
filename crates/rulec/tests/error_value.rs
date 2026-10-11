//! The Python module holds the value it refused as a string, and nothing it generates for the
//! error is a type that takes "any object".
//!
//! `RuleInputError` used to take any object as its `value` and compared it with a sentinel
//! made by `object()` to tell "no value" from a value. Code that is compiled ahead of time
//! has neither: no type for anything, no statement that runs when the module is loaded. The
//! value is now the `repr()` of what was refused, taken where the refusal is made, and
//! `None` when the refusal is about no one value (§15.207). The message, `str(error)`, is the
//! one it was.
//!
//! Three things are held here. The generated files pass nothing but a `repr` of it to the error,
//! and the module has no `object`, no sentinel and — unless the rule reads a caller's
//! object — no `Any`. The module's own refusals say what they said, with the value as a
//! string. And so do the MCP server's, which read the wire with a reader of their own.
//!
//! The materials are three rules of the tree, English first with the Japanese version of each
//! beside it: an amount and a mass (`member_shipping_fee`, `送料`), a fare with a `constraint`
//! between two amounts, so that the door has a refusal about no one value
//! (`express_delivery_quote`, `速達の見積`), and a count over the elements of a walk
//! (`supplier_matching`, `納入先照合`).

use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd).arg(arg).output().map(|o| o.status.success()).unwrap_or(false)
}

/// What one language of the materials is called, and what it says: the sentences are the ones
/// the rules generate in that language.
struct Case {
    lang: &'static str,
    /// The three rules: the shipping fee, the express fare and the supplier matching.
    rules: [&'static str; 3],
    /// The module each of them is generated as.
    ship: &'static str,
    express: &'static str,
    suppliers: &'static str,
    /// The sentences of the shipping fee's weight: not an integer, out of range.
    not_integer: &'static str,
    /// The same of the fare's first input, which the Japanese version calls another name.
    fare_not_integer: &'static str,
    /// The same of the fare's third input.
    declared_not_integer: &'static str,
    out_of_range: &'static str,
    /// The destination of the shipping fee is not a member of its enum.
    not_in_enum: &'static str,
    /// The fare's `constraint`, which is about no one value.
    constraint: &'static str,
    /// The fare's `express` is not a truth value.
    not_bool: &'static str,
    /// The supplier matching was handed more candidates than it takes.
    too_many: &'static str,
}

const CASES: [Case; 2] = [
    Case {
        lang: "en",
        rules: ["tests/corpus/member_shipping_fee.rule", "tests/corpus/express_delivery_quote.rule", "tests/corpus/supplier_matching.rule"],
        ship: "member_shipping_fee",
        express: "express_delivery_quote",
        suppliers: "supplier_matching",
        not_integer: "weight is not an integer",
        fare_not_integer: "weight is not an integer",
        declared_not_integer: "declared is not an integer",
        out_of_range: "weight is out of range",
        not_in_enum: "dest is not a value of enum jp_prefecture",
        constraint: "the constraint does not hold: declared <= cover",
        not_bool: "express is not a boolean",
        too_many: "candidates has too many elements (at most 50)",
    },
    Case {
        lang: "ja",
        rules: ["tests/corpus/送料.rule", "tests/corpus/速達の見積.rule", "tests/corpus/納入先照合.rule"],
        ship: "shipping_fee",
        express: "express_quote",
        suppliers: "supplier_match",
        not_integer: "重量 が整数ではありません",
        fare_not_integer: "重さ が整数ではありません",
        declared_not_integer: "申告額 が整数ではありません",
        out_of_range: "重量 が範囲の外です",
        not_in_enum: "届け先 が列挙 都道府県 の値ではありません",
        constraint: "制約が成り立ちません: 申告額 <= 補償額",
        not_bool: "速達 が真偽ではありません",
        too_many: "候補 の要素が多すぎます（上限 50）",
    },
];

/// `rulec gen` of the three rules of a case into a directory of its own.
fn generate(c: &Case) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("error-value-{}", c.lang));
    let out = t.path().join("out");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rulec"));
    cmd.env("RULEC_LANG", c.lang).current_dir(root()).arg("gen").args(c.rules).arg("--out").arg(&out);
    let o = cmd.output().expect("cannot start rulec");
    assert!(o.status.success(), "{}: {}{}", c.lang, String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    (t, out)
}

fn read(dir: &Path, file: &str) -> String {
    std::fs::read_to_string(dir.join("python").join(file)).unwrap_or_else(|e| panic!("cannot read python/{file}: {e}"))
}

/// The second argument of a call of `RuleInputError(…)` on this line, if it has one: what comes
/// after the first argument (the sentence, a string or an f-string) up to the call's own
/// closing parenthesis.
fn value_argument(line: &str) -> Option<String> {
    let at = line.find("RuleInputError(")? + "RuleInputError(".len();
    let b: Vec<char> = line[at..].chars().collect();
    let (mut i, mut depth, mut quote) = (0, 1, None::<char>);
    let mut comma = None;
    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                if c == '\\' {
                    i += 1;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return comma.map(|k: usize| b[k + 1..i].iter().collect::<String>().trim().to_string());
                    }
                }
                ',' if depth == 1 && comma.is_none() => comma = Some(i),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// The reader of the argument finds it past a first argument that has a comma, a parenthesis or
/// a brace in it, and finds none where there is only the sentence; the test below is as good as
/// this is.
#[test]
fn the_value_argument_is_found_past_the_sentence() {
    assert_eq!(value_argument(r#"        raise RuleInputError("a, (b", repr(x))"#).as_deref(), Some("repr(x)"));
    assert_eq!(value_argument(r#"        raise m.RuleInputError(f"{name} is not an integer", repr(len(v))) from None"#).as_deref(), Some("repr(len(v))"));
    assert_eq!(value_argument(r#"        raise RuleInputError("a", v)"#).as_deref(), Some("v"));
    assert_eq!(value_argument(r#"        raise RuleInputError("only a sentence")"#), None);
    assert_eq!(value_argument("        raise AssertionError(\"not this\", 1)"), None);
}

/// The module has no `object`, no sentinel and no `Any` where it reads nothing from a caller's
/// object; the error takes a `str` or nothing; and every file that raises it hands over a
/// `repr(…)` and nothing else (`_repr(…)` in the module, which binds the builtin to that name).
#[test]
fn the_files_pass_the_error_nothing_but_a_repr() {
    for c in &CASES {
        let (_t, out) = generate(c);
        for (rule, module) in c.rules.iter().zip([c.ship, c.express, c.suppliers]) {
            let m = read(&out, &format!("{module}.py"));
            assert!(m.contains("class RuleInputError(ValueError):"), "{rule}");
            assert!(m.contains("def __init__(self, what: str, value: str | None = None) -> None:"), "{rule}: the value is not a str or None");
            assert!(m.contains("return self.what if self.value is None else f\"{self.what}: {self.value}\""), "{rule}");
            // `Any` is what a caller's object is read as (`<alias>_from`), and nothing else is.
            let reads_a_callers_object = m.contains(&format!("def {module}_from("));
            let words = |w: &str| m.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')).filter(|x| *x == w).count();
            assert_eq!(words("Any") > 0, reads_a_callers_object, "{rule}: Any");
            // (The word is in the docstring of `<alias>_from`, so it is the type that is looked for.)
            assert!(!m.contains("object()") && !m.contains(": object") && !m.contains("[object"), "{rule}: object");
            assert_eq!(words("_NOVALUE"), 0, "{rule}: the sentinel");
            // Every file that raises the error: the module, the runner, the Connect runner and
            // the MCP server. (The server's reader is a file of its own.)
            let mut raised = 0;
            // (The module calls the builtin by the name it binds, `_repr`; the others are programs of
            // their own, in which no name of the rule is in scope.)
            for (file, repr) in [
                (format!("{module}.py"), "_repr("),
                (format!("{module}_runner.py"), "repr("),
                (format!("{module}_connect_runner.py"), "repr("),
                (format!("{module}_mcp.py"), "repr("),
            ] {
                let text = read(&out, &file);
                assert_eq!(text.matches("_NOVALUE").count(), 0, "{rule}: {file}");
                for l in text.lines().filter(|l| l.contains("raise ") && l.contains("RuleInputError(")) {
                    if let Some(v) = value_argument(l) {
                        raised += 1;
                        assert!(v.starts_with(repr) && v.ends_with(')'), "{rule}: {file}: the error is handed `{v}`, not a repr:\n{l}");
                    }
                }
            }
            assert!(raised > 0, "{rule}: no refusal with a value was found");
        }
    }
}

/// Python, handed what the doors refuse, says what it said, and holds the value as the string
/// `repr()` makes of it: `None` for a refusal about no one value, and the string `'None'` — not
/// `None` — for a refused value that is `None`.
#[test]
fn a_refusal_says_what_it_said_and_holds_the_value_as_a_string() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here; the modules are not called") {
        return;
    }
    for c in &CASES {
        let (_t, out) = generate(c);
        let script = format!(
            "import {ship} as s, {express} as x, {suppliers} as m\n\
             def refused(mod, fn, *a):\n\
             \x20   try:\n\
             \x20       fn(*a)\n\
             \x20   except mod.RuleInputError as e:\n\
             \x20       assert e.value is None or type(e.value) is str, e.value\n\
             \x20       return str(e), e.what, e.value\n\
             \x20   raise SystemExit('answered: ' + repr(a))\n\
             P, K = s.Prefecture.HOKKAIDO, s.MemberKind.GOLD\n\
             ni, oor, ne = {not_integer:?}, {out_of_range:?}, {not_in_enum:?}\n\
             # a value that is refused: the sentence, then the value after it, and the value as repr says it\n\
             assert refused(s, s.{ship}, P, 2500.5, 12000, K) == (ni + ': 2500.5', ni, '2500.5')\n\
             assert refused(s, s.{ship}, P, '1000', 12000, K) == (ni + \": '1000'\", ni, \"'1000'\")\n\
             assert refused(s, s.{ship}, P, True, 12000, K) == (ni + ': True', ni, 'True')\n\
             assert refused(s, s.{ship}, P, 10**9, 12000, K) == (oor + ': 1000000000', oor, '1000000000')\n\
             assert refused(s, s.{ship}, 'hokkaido', 2500, 12000, K) == (ne + \": 'hokkaido'\", ne, \"'hokkaido'\")\n\
             # a refused value that is None is a value: the string, not the mark of none\n\
             assert refused(s, s.{ship}, None, 2500, 12000, K) == (ne + ': None', ne, 'None')\n\
             # a refusal about no one value: the constraint between two inputs\n\
             c = {constraint:?}\n\
             assert refused(x, x.{express}, 1000, False, 200000, 100000) == (c, c, None)\n\
             nb = {not_bool:?}\n\
             assert refused(x, x.{express}, 1000, 'yes', 0, 0) == (nb + \": 'yes'\", nb, \"'yes'\")\n\
             # the elements of a walk, and the length of the list\n\
             tm = {too_many:?}\n\
             assert refused(m, m.{suppliers}, True, [m.Element(True, False)] * 300) == (tm + ': 300', tm, '300')\n\
             # the error built by hand\n\
             assert (str(s.RuleInputError('x')), s.RuleInputError('x').value) == ('x', None)\n\
             assert (str(s.RuleInputError('x', '5')), s.RuleInputError('x', '5').value) == ('x: 5', '5')\n\
             print('error value ok')\n",
            ship = c.ship,
            express = c.express,
            suppliers = c.suppliers,
            not_integer = c.not_integer,
            out_of_range = c.out_of_range,
            not_in_enum = c.not_in_enum,
            constraint = c.constraint,
            not_bool = c.not_bool,
            too_many = c.too_many,
        );
        let o = Command::new("python3").current_dir(out.join("python")).env("PYTHONDONTWRITEBYTECODE", "1").args(["-B", "-X", "utf8", "-c", &script]).output().unwrap();
        assert!(o.status.success(), "{}:\n{}{}", c.lang, String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "error value ok", "{}", c.lang);
    }
}

/// The MCP server reads the wire with readers of its own and hands the module's error the value
/// as `repr()` says it; what it answers is the module's message, as before.
#[test]
fn the_mcp_server_answers_with_the_same_message() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here; the server is not called") {
        return;
    }
    for c in &CASES {
        let (_t, out) = generate(c);
        let script = format!(
            "import {express}_mcp as mcp\n\
             names = list(mcp.TOOL['inputSchema']['properties'])\n\
             def said(*values):\n\
             \x20   args = dict(zip(names, values))\n\
             \x20   r = mcp.handle({{'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call', 'params': {{'name': mcp.TOOL['name'], 'arguments': args}}}})\n\
             \x20   assert r['result']['isError'], r\n\
             \x20   return r['result']['content'][0]['text']\n\
             nb, ni = {not_bool:?}, {not_integer:?}\n\
             assert said(1000, 'yes', 0, 0) == 'RuleInputError: ' + nb + \": 'yes'\"\n\
             assert said(1000.5, False, 0, 0) == 'RuleInputError: ' + ni + ': 1000.5'\n\
             assert said(1000, False, '0', 0) == 'RuleInputError: ' + {declared_not_integer:?} + \": '0'\"\n\
             c = {constraint:?}\n\
             assert said(1000, False, 200000, 100000) == 'RuleInputError: ' + c\n\
             print('mcp ok')\n",
            express = c.express,
            not_bool = c.not_bool,
            not_integer = c.fare_not_integer,
            declared_not_integer = c.declared_not_integer,
            constraint = c.constraint,
        );
        let o = Command::new("python3").current_dir(out.join("python")).env("PYTHONDONTWRITEBYTECODE", "1").args(["-B", "-X", "utf8", "-c", &script]).output().unwrap();
        assert!(o.status.success(), "{}:\n{}{}", c.lang, String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "mcp ok", "{}", c.lang);
    }
}
