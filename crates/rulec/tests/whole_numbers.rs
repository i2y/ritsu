//! A number whose value is whole is the integer it is, at every door (§15.205).
//!
//! JSON does not tell `1000.0` from `1000`, and a caller's `float` writes the first: Python's
//! `json.dumps(1000.0)` does, and so does a sheet that exports its numbers as decimals. Every door
//! but NumPy's refused it as not an integer — and the TypeScript and JavaScript ones, whose
//! `JSON.parse` cannot tell either — and so did the reference evaluator. Now every door reads
//! `1000.0`, `1e3`, `1.5e3` and `-0.0` as `1000`, `1000`, `1500` and `0`, and holds the integer to
//! its range and its step as before; `1000.5` is still not an integer, and `"1000"` still not a
//! number.
//!
//! The materials are rules of the tree with numbers of every kind, English first, with the
//! Japanese version of each beside it: an amount and a mass (`member_shipping_fee`, `送料`), rates
//! in steps of 0.01% (`health_insurance_premium`, `健康保険料`), optional numbers under a
//! constraint (`parcel_cover`, `小包の補償`), amounts in the elements of a walk
//! (`nationwide_freight`, `全国運賃`) and a sum over a sequence (`cart_shipping_fee`,
//! `買物かごの送料`).

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, ready, skip};
use rulec::json::Json;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const ENGLISH: [&str; 5] = [
    "tests/corpus/member_shipping_fee.rule",
    "tests/corpus/health_insurance_premium.rule",
    "tests/optional/parcel_cover.rule",
    "tests/corpus/nationwide_freight.rule",
    "tests/corpus/cart_shipping_fee.rule",
];

const JAPANESE: [&str; 5] = [
    "tests/corpus/送料.rule",
    "tests/corpus/健康保険料.rule",
    "tests/optional/小包の補償.rule",
    "tests/corpus/全国運賃.rule",
    "tests/corpus/買物かごの送料.rule",
];

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd).arg(arg).output().map(|o| o.status.success()).unwrap_or(false)
}

fn rulec(lang: &str, tmp: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).current_dir(root()).args(args);
    if let Some(t) = tmp {
        c.env("TMPDIR", t);
    }
    let o = c.output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `rulec gen` of a set of rules into one directory, in a language.
fn generate(rules: &[&str], lang: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new("whole");
    let out = t.path().join("out");
    let mut args = vec!["gen"];
    args.extend(rules.iter().copied());
    let o = out.to_string_lossy().into_owned();
    args.extend(["--out", o.as_str()]);
    let (code, said, e) = rulec(lang, None, &args);
    assert_eq!(code, 0, "{said}{e}");
    (t, out)
}

/// Every whole-number spelling of `n` this test writes: with a point, with an exponent, with
/// both, in capitals, with a plus, and zero with a minus.
fn spellings(n: i128) -> Vec<String> {
    let mut out = vec![format!("{n}.0"), format!("{n}e0"), format!("{n}.000")];
    if n == 0 {
        out.extend(["-0.0".to_string(), "0e5".to_string(), "0.0e-3".to_string()]);
        return out;
    }
    let (mut m, mut k) = (n, 0);
    while m % 10 == 0 {
        m /= 10;
        k += 1;
    }
    if k > 0 {
        out.extend([format!("{m}e{k}"), format!("{m}E+{k}")]);
    }
    let digits = n.unsigned_abs().to_string();
    if digits.len() > 1 {
        let sign = if n < 0 { "-" } else { "" };
        out.push(format!("{sign}{}.{}e{}", &digits[..1], &digits[1..], digits.len() - 1));
    }
    out
}

/// A value written back with every integer in it spelled another whole-number way, the next
/// spelling each time, so that the vectors of a rule between them use all of them.
fn respell(j: &Json, k: &mut usize, used: &mut BTreeMap<String, usize>) -> String {
    match j {
        Json::Int(n) => {
            let all = spellings(*n);
            let s = all[*k % all.len()].clone();
            *k += 1;
            let kind = if s.starts_with("-0") {
                "minus zero"
            } else if s.contains(['e', 'E']) && s.contains('.') {
                "point and exponent"
            } else if s.contains(['e', 'E']) {
                "exponent"
            } else {
                "point"
            };
            *used.entry(kind.to_string()).or_default() += 1;
            s
        }
        Json::Arr(xs) => format!("[{}]", xs.iter().map(|x| respell(x, k, used)).collect::<Vec<_>>().join(",")),
        Json::Obj(m) => format!(
            "{{{}}}",
            m.iter().map(|(key, v)| format!("{}:{}", rulec::json::quote(key), respell(v, k, used))).collect::<Vec<_>>().join(",")
        ),
        other => rulec::json::unparse(other),
    }
}

/// Every vectors file under `out` with the integers of its inputs spelled the other ways. What
/// each case answers is left as it is: the answer has to be the same.
fn respell_vectors(out: &Path) -> BTreeMap<String, usize> {
    let mut used = BTreeMap::new();
    let mut k = 0usize;
    for e in std::fs::read_dir(out.join("vectors")).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        // `<alias>.jsonl` alone: not the expected answers, the refused inputs or the traces.
        if !name.ends_with(".jsonl") || name.trim_end_matches(".jsonl").contains('.') {
            continue;
        }
        let body = std::fs::read_to_string(e.path()).unwrap();
        let mut lines = Vec::new();
        for l in body.lines().filter(|l| !l.trim().is_empty()) {
            let Json::Obj(m) = rulec::json::parse(l).unwrap_or_else(|x| panic!("{name}: {x}")) else { panic!("{name}: {l}") };
            let fields: Vec<String> = m
                .iter()
                .map(|(key, v)| {
                    let v = if key == "in" { respell(v, &mut k, &mut used) } else { rulec::json::unparse(v) };
                    format!("{}:{v}", rulec::json::quote(key))
                })
                .collect();
            lines.push(format!("{{{}}}", fields.join(",")));
        }
        std::fs::write(e.path(), lines.join("\n") + "\n").unwrap();
    }
    used
}

/// `rulec test` over a generated directory, as JSON, with a SKIP line for whatever it did not run.
fn rulec_test(out: &Path) -> (i32, Json) {
    let (code, said, e) = rulec("en", Some(&tmpdir_in(out)), &["test", &out.to_string_lossy(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    (code, j)
}

/// Every way `rulec test` reaches the rules — the runners of the twelve languages, Rust's WASI
/// module, the Connect service, the query and the function — reads every integer of the vectors
/// written as `1000.0`, `1e3`, `1.5e3` or `-0.0` as the integer it is, and answers each case as it
/// answers the integer: the same record, byte for byte, the inputs in it written as integers.
#[test]
fn every_way_answers_a_whole_number_in_any_spelling_as_the_integer() {
    // Every language there is a toolchain for, the tools level's (ritsu's DESIGN 10.2).
    if !need(Need::Python) {
        return;
    }
    for (rules, lang) in [(&ENGLISH, "en"), (&JAPANESE, "ja")] {
        let (_t, out) = generate(rules, lang);
        let used = respell_vectors(&out);
        for kind in ["point", "exponent", "point and exponent", "minus zero"] {
            assert!(used.get(kind).is_some_and(|n| *n > 0), "[{lang}] no number was written with a {kind}: {used:?}");
        }
        let (code, j) = rulec_test(&out);
        let mut ways: BTreeMap<String, usize> = BTreeMap::new();
        for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if r.get("ran").and_then(|v| v.as_bool()) != Some(true) || r.get("via").and_then(|v| v.as_str()) == Some("proof") {
                continue;
            }
            let alias = r.get("rule").and_then(|v| v.as_str()).unwrap_or_default();
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{alias} [{lang}]: {r:?}");
            let way = format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default());
            *ways.entry(way).or_default() += 1;
        }
        assert_eq!(code, 0, "{j:?}");
        for way in ["python/runner", "numpy/runner", "typescript/runner", "javascript/runner", "rust/runner", "ruby/runner", "sql/runner"] {
            assert!(ways.contains_key(way), "[{lang}] {way} did not run: {ways:?}");
        }
        println!("whole numbers [{lang}]: {used:?} through {} ways: {}", ways.len(), ways.keys().cloned().collect::<Vec<_>>().join(", "));
    }
}

/// Send the requests one line at a time and collect one answer per request that has an id.
fn talk(cwd: &Path, cmd: &str, args: &[&str], reqs: &[String]) -> Vec<Json> {
    let mut child = Command::new(cmd)
        .current_dir(cwd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start the server");
    let mut si = child.stdin.take().unwrap();
    let mut so = BufReader::new(child.stdout.take().unwrap());
    let mut out = Vec::new();
    for r in reqs {
        writeln!(si, "{r}").unwrap();
        si.flush().unwrap();
        if !r.contains("\"id\"") {
            continue;
        }
        let mut line = String::new();
        assert!(so.read_line(&mut line).unwrap() > 0, "ended without an answer: {r}");
        out.push(rulec::json::parse(line.trim()).unwrap_or_else(|e| panic!("not JSON: {e}\n{line}")));
    }
    drop(si);
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "the server failed:\n{}", String::from_utf8_lossy(&o.stderr));
    out
}

/// The text of a `tools/call` answer, and whether it is an error.
fn answer(j: &Json) -> (String, bool) {
    let r = j.get("result").unwrap_or_else(|| panic!("no result: {}", rulec::json::unparse(j)));
    let text = match r.get("content") {
        Some(Json::Arr(a)) => a.first().and_then(|c| c.get("text")).and_then(|t| t.as_str()).unwrap_or_default().to_string(),
        _ => String::new(),
    };
    (text, r.get("isError") == Some(&Json::Bool(true)))
}

/// The generated MCP servers — Python's, and the TypeScript and JavaScript ones — take an
/// argument written `2500.0` or `1.2e4` as the integer, and answer with the record they give the
/// integer; `2500.5` is still refused as not an integer.
#[test]
fn the_mcp_servers_take_a_whole_number_as_the_integer() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here; the Python server is not asked") {
        return;
    }
    let node = ready(Need::Node, || have("node", "--version"), "node is not here; the TypeScript and JavaScript servers are not asked");
    // The English rule first, the Japanese version beside it: the tool, the names and values its
    // arguments are written with on the wire, and the sentence of a fraction.
    for (rule, lang, alias, (dest, weight_key, total_key, member), not_integer) in [
        (
            "tests/corpus/member_shipping_fee.rule",
            "en",
            "member_shipping_fee",
            (r#""dest":"Hokkaido""#, "weight", "total", r#""member":"gold""#),
            "weight is not an integer",
        ),
        ("tests/corpus/送料.rule", "ja", "shipping_fee", (r#""届け先":"北海道""#, "重量", "注文金額", r#""会員":"ゴールド""#), "重量 が整数ではありません"),
    ] {
        let (_t, out) = generate(&[rule], lang);
        let call = |id: u32, weight: &str, total: &str| {
            format!(
                "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"tools/call\",\"params\":{{\"name\":\"{alias}\",\"arguments\":{{{dest},\"{weight_key}\":{weight},\"{total_key}\":{total},{member}}}}}}}"
            )
        };
        let reqs = [
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"t\",\"version\":\"0\"}}}".to_string(),
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}".to_string(),
            call(2, "2500", "12000"),
            call(3, "2500.0", "1.2e4"),
            call(4, "25e2", "-0.0"),
            call(5, "2500", "0"),
            call(6, "2500.5", "12000"),
        ];
        let mut servers: Vec<(&str, &str, Vec<String>)> = vec![("python", "python3", vec!["-B".into(), format!("{alias}_mcp.py")])];
        if node {
            servers.push(("javascript", "node", vec![format!("{alias}_mcp.mjs")]));
            servers.push(("typescript", "node", vec!["--no-warnings".into(), format!("{alias}_mcp.ts")]));
        }
        for (dir, cmd, args) in servers {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let a = talk(&out.join(dir), cmd, &args, &reqs);
            let (int, int_err) = answer(&a[1]);
            assert!(!int_err && int.contains(&format!("\"{weight_key}\":2500")), "{rule} {dir}: {int}");
            let (whole, whole_err) = answer(&a[2]);
            assert!(!whole_err, "{rule} {dir}: 2500.0 and 1.2e4 were refused: {whole}");
            assert_eq!(whole, int, "{rule} {dir}: 2500.0 and 1.2e4 are not answered as 2500 and 12000");
            let (zero, zero_err) = answer(&a[3]);
            let (zero_int, _) = answer(&a[4]);
            assert!(!zero_err && zero == zero_int, "{rule} {dir}: 25e2 and -0.0 are not 2500 and 0: {zero} / {zero_int}");
            let (frac, frac_err) = answer(&a[5]);
            assert!(frac_err && frac.contains(not_integer), "{rule} {dir}: 2500.5 was not refused as not an integer: {frac}");
        }
    }
}

/// Run a script in a language, in a directory, and give back what it printed.
fn script(dir: &Path, cmd: &str, args: &[&str]) -> String {
    let o = Command::new(cmd).current_dir(dir).args(args).output().unwrap_or_else(|e| panic!("cannot start {cmd}: {e}"));
    assert!(o.status.success(), "{cmd} {args:?} failed:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// The modules of the languages a caller can pass anything to — Python, JavaScript and Ruby —
/// take a whole float (a whole number, in JavaScript) as the integer at the door of an input and
/// at the door of an element, answer as they answer the integer, and still refuse `2500.5`.
#[test]
fn the_module_doors_take_a_whole_number_as_the_integer() {
    if !ready(Need::Python, || have("python3", "--version"), "python3 is not here; the modules are not called") {
        return;
    }
    // The English rules first, the Japanese versions beside them: (shipping module and function,
    // walk module and function, the sentence of a fraction, and the two as Ruby names them).
    for (lang, rules, ship, walk, not_integer, (s, w)) in [
        (
            "en",
            ["tests/corpus/member_shipping_fee.rule", "tests/corpus/nationwide_freight.rule"],
            "member_shipping_fee",
            "nationwide_freight",
            "weight is not an integer",
            ("MemberShippingFee", "NationwideFreight"),
        ),
        ("ja", ["tests/corpus/送料.rule", "tests/corpus/全国運賃.rule"], "shipping_fee", "freight", "重量 が整数ではありません", ("ShippingFee", "Freight")),
    ] {
        let (_t, out) = generate(&rules, lang);
        // Python: a float at an input, and in a NamedTuple element.
        let py = format!(
            "import {ship} as s, {walk} as w\n\
             a = s.{ship}(s.Prefecture.HOKKAIDO, 2500, 12000, s.MemberKind.GOLD)\n\
             b = s.{ship}(s.Prefecture.HOKKAIDO, 2500.0, 1.2e4, s.MemberKind.GOLD)\n\
             c = s.{ship}(s.Prefecture.HOKKAIDO, 25e2, -0.0, s.MemberKind.GOLD)\n\
             d = s.{ship}(s.Prefecture.HOKKAIDO, 2500, 0, s.MemberKind.GOLD)\n\
             assert a == b and type(b) is int and c == d, (a, b, c, d)\n\
             try:\n    s.{ship}(s.Prefecture.HOKKAIDO, 2500.5, 12000, s.MemberKind.GOLD)\n    raise SystemExit('2500.5 answered')\n\
             except s.RuleInputError as e:\n    assert e.what == {not_integer:?}, e.what\n\
             x = w.{walk}([w.Element(w.Zone.KINKI, 1000, 500), w.Element(w.Zone.KINKI, 2000, 700)])\n\
             y = w.{walk}([w.Element(w.Zone.KINKI, 1000.0, 5e2), w.Element(w.Zone.KINKI, 2e3, 700.0)])\n\
             assert x == y and type(y) is int, (x, y)\n\
             print('python', a, c, x)\n"
        );
        let said = script(&out.join("python"), "python3", &["-B", "-c", &py]);
        assert!(said.starts_with("python "), "{said}");
        if ready(Need::Node, || have("node", "--version"), "node is not here; the JavaScript module is not called") {
            let js = format!(
                "import * as s from './{ship}.mjs';\n\
                 import * as w from './{walk}.mjs';\n\
                 const a = s.{ship}(s.Prefecture.HOKKAIDO, 2500n, 12000n, s.MemberKind.GOLD);\n\
                 const b = s.{ship}(s.Prefecture.HOKKAIDO, 2500, 1.2e4, s.MemberKind.GOLD);\n\
                 const c = s.{ship}(s.Prefecture.HOKKAIDO, 25e2, -0, s.MemberKind.GOLD);\n\
                 const d = s.{ship}(s.Prefecture.HOKKAIDO, 2500n, 0n, s.MemberKind.GOLD);\n\
                 if (a !== b || typeof b !== 'bigint' || c !== d) throw new Error(`${{a}} ${{b}} ${{c}} ${{d}}`);\n\
                 let refused = false;\n\
                 try {{ s.{ship}(s.Prefecture.HOKKAIDO, 2500.5, 12000n, s.MemberKind.GOLD); }} catch (e) {{ refused = e.what === {not_integer:?}; }}\n\
                 if (!refused) throw new Error('2500.5 was not refused');\n\
                 const x = w.{walk}([{{ row_zone: w.Zone.KINKI, threshold: 1000n, row_fee: 500n }}, {{ row_zone: w.Zone.KINKI, threshold: 2000n, row_fee: 700n }}]);\n\
                 const y = w.{walk}([{{ row_zone: w.Zone.KINKI, threshold: 1000, row_fee: 5e2 }}, {{ row_zone: w.Zone.KINKI, threshold: 2e3, row_fee: 700 }}]);\n\
                 if (x !== y) throw new Error(`${{x}} ${{y}}`);\n\
                 console.log('javascript', String(a), String(x));\n"
            );
            let path = out.join("javascript").join("whole.mjs");
            std::fs::write(&path, js).unwrap();
            let said = script(&out.join("javascript"), "node", &["whole.mjs"]);
            assert!(said.starts_with("javascript "), "{said}");
        }
        if ready(Need::Ruby, || have("ruby", "--version"), "ruby is not here; the Ruby module is not called") {
            let rb = format!(
                "require_relative '{ship}'\n\
                 require_relative '{walk}'\n\
                 a = {s}.{ship}({s}::Prefecture::HOKKAIDO, 2500, 12000, {s}::MemberKind::GOLD)\n\
                 b = {s}.{ship}({s}::Prefecture::HOKKAIDO, 2500.0, 1.2e4, {s}::MemberKind::GOLD)\n\
                 c = {s}.{ship}({s}::Prefecture::HOKKAIDO, 25e2, -0.0, {s}::MemberKind::GOLD)\n\
                 d = {s}.{ship}({s}::Prefecture::HOKKAIDO, 2500, 0, {s}::MemberKind::GOLD)\n\
                 raise \"#{{a}} #{{b}} #{{c}}\" unless a == b && b.is_a?(Integer) && c == d\n\
                 begin\n  {s}.{ship}({s}::Prefecture::HOKKAIDO, 2500.5, 12000, {s}::MemberKind::GOLD)\n  raise '2500.5 answered'\n\
                 rescue {s}::RuleInputError => e\n  raise e.what unless e.what == {not_integer:?}\nend\n\
                 x = {w}.{walk}([{w}::Element.new({w}::Zone::KINKI, 1000, 500), {w}::Element.new({w}::Zone::KINKI, 2000, 700)])\n\
                 y = {w}.{walk}([{w}::Element.new({w}::Zone::KINKI, 1000.0, 5e2), {w}::Element.new({w}::Zone::KINKI, 2e3, 700.0)])\n\
                 raise \"#{{x}} #{{y}}\" unless x == y && y.is_a?(Integer)\n\
                 puts \"ruby #{{a}} #{{x}}\"\n"
            );
            let said = script(&out.join("ruby"), "ruby", &["-e", &rb]);
            assert!(said.starts_with("ruby "), "{said}");
        }
    }
}

/// The reference evaluator reads a record's `1000.0` as the integer too: `fixtures lint` takes a
/// record whose numbers are written with a point, and still says a fraction is not the integer
/// the field wants. The file of refused inputs keeps its numbers that are not whole.
#[test]
fn the_reference_reads_a_whole_number_as_the_integer() {
    // The English rule first, the Japanese version beside it: a record's inputs and output by
    // the names the wire writes them with, the name of the mass, and what each says of a fraction.
    for (rule, lang, alias, (head, weight_key, total_key, tail), fraction, not_integer) in [
        (
            "tests/corpus/member_shipping_fee.rule",
            "en",
            "member_shipping_fee",
            (r#""dest":"Hokkaido""#, "weight", "total", r#""member":"gold"},"observed":{"fee":1800}"#),
            "found a fraction",
            "weight is not an integer",
        ),
        (
            "tests/corpus/送料.rule",
            "ja",
            "shipping_fee",
            (r#""届け先":"北海道""#, "重量", "注文金額", r#""会員":"ゴールド"},"observed":{"送料":1800}"#),
            "小数 でした",
            "重量 が整数ではありません",
        ),
    ] {
        let t = TempDir::new("whole-fixtures");
        let records = t.path().join("records.jsonl");
        let record = |weight: &str, total: &str| format!("{{\"in\":{{{head},\"{weight_key}\":{weight},\"{total_key}\":{total},{tail}}}");
        let ok = record("2500.0", "-0.0");
        std::fs::write(&records, format!("{ok}\n")).unwrap();
        let (code, said, e) = rulec(lang, None, &["fixtures", "lint", &records.to_string_lossy(), rule]);
        assert_eq!(code, 0, "{rule}: a record of whole numbers written with a point was not taken:\n{said}{e}");
        let bad = record("2500.5", "0");
        std::fs::write(&records, format!("{bad}\n")).unwrap();
        let (code, said, _) = rulec(lang, None, &["fixtures", "lint", &records.to_string_lossy(), rule]);
        assert_eq!(code, 1, "{rule}: 2500.5 was taken:\n{said}");
        assert!(said.contains(fraction), "{rule}: {said}");
        // `gen` still writes a number that is not whole among the refused inputs, refused as not
        // an integer.
        let (_g, out) = generate(&[rule], lang);
        let body = std::fs::read_to_string(out.join("vectors").join(format!("{alias}.refused.jsonl"))).unwrap();
        let fractions = body
            .lines()
            .filter_map(|l| rulec::json::parse(l).ok())
            .filter(|j| j.get("error").and_then(|e| e.as_str()) == Some(not_integer))
            .filter(|j| matches!(j.get("in").and_then(|i| i.get(weight_key)), Some(Json::Frac(f)) if f.ends_with(".5")))
            .count();
        assert_eq!(fractions, 1, "{rule}: no number that is not whole among the refused:\n{body}");
    }
}
