//! The projection function, **run** (§15.125).
//!
//! `rulec test` compiles the generated module and runs its vectors, and the vectors are the
//! rule's own flat inputs — so the projection function is compiled and never called. A
//! generated function nothing runs is a generated function nothing checks, which is the
//! reason §15.46 keeps targets it cannot run out of the set altogether.
//!
//! So one object, built by hand against the contract the corpus rule cites, is pushed
//! through every language that generates a projection, and the answer is held to the one the
//! rule gives for the same case directly. A language whose toolchain is missing is skipped
//! and said so, the way `rulec test` does; Python is required, because a run with nothing in
//! it would pass while proving nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// The corpus rule that projects, generated once.
fn generated(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-projection-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", "tests/corpus/注文の送料.rule", "--out", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    dir
}

/// One order: Okinawa, eleven lines, one of them chilled. The rule's own `examples` pin the
/// same case as `okinawa | true | 11 -> 2200円`, so what this holds the five projections to
/// is a number the reference evaluator already agreed with.
const WANT: &str = "2200";

fn lines_json() -> String {
    let mut v: Vec<String> = Vec::new();
    for i in 0..11 {
        let chilled = if i == 3 { "true" } else { "false" };
        v.push(format!("{{\"sku\":\"s{i}\",\"chilled\":{chilled},\"amount_jpy\":100}}"));
    }
    format!("[{}]", v.join(","))
}

fn order_json() -> String {
    format!("{{\"shipping\":{{\"zone\":\"okinawa\",\"postcode\":\"900-0001\"}},\"lines\":{}}}", lines_json())
}

/// Run one driver and hold what it prints to `WANT`.
fn check(lang: &str, cmd: &mut Command) {
    let o = cmd.output().unwrap_or_else(|e| panic!("{lang} を起動できない: {e}"));
    let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
    assert!(
        o.status.success(),
        "{lang}: 射影が走らない:\n{}\n{}",
        out,
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(out, WANT, "{lang}: 射影の答えが違う");
}

#[test]
fn 射影は五つの言語で同じ答えを出す() {
    let dir = generated("run");
    let mut ran = 0;

    // --- Python
    assert!(have("python3"), "python3 が要ります（この試験は生成物を走らせます）");
    let script = format!(
        "import json, order_shipping as m\nprint(m.order_shipping_from(json.loads({:?})))\n",
        order_json()
    );
    check("Python", Command::new("python3").current_dir(dir.join("python")).args(["-B", "-c", &script]));
    ran += 1;

    // --- JavaScript, and with it the TypeScript it is stripped from.
    if have("node") {
        let script = format!(
            "import('./order_shipping.mjs').then(m => console.log(String(m.order_shipping_from(JSON.parse({:?})))));",
            order_json()
        );
        check("JavaScript", Command::new("node").current_dir(dir.join("javascript")).args(["--input-type=module", "-e", &script]));
        ran += 1;
    } else {
        eprintln!("注意: node が無いので JavaScript 側を飛ばした");
    }

    // --- Ruby
    if have("ruby") {
        let script = format!(
            "require 'json'\nrequire './order_shipping.rb'\nputs OrderShipping.order_shipping_from(JSON.parse({:?}))\n",
            order_json()
        );
        check("Ruby", Command::new("ruby").current_dir(dir.join("ruby")).args(["-e", &script]));
        ran += 1;
    } else {
        eprintln!("注意: ruby が無いので Ruby 側を飛ばした");
    }

    // --- PHP
    if have("php") {
        let script = format!(
            "require './order_shipping.php'; echo \\OrderShipping\\order_shipping_from(json_decode({}, true));",
            php_str(&order_json())
        );
        check("PHP", Command::new("php").current_dir(dir.join("php")).args(["-r", &script]));
        ran += 1;
    } else {
        eprintln!("注意: php が無いので PHP 側を飛ばした");
    }

    // --- TypeScript. Node reads a `.ts` by stripping the types, which is how `rulec test`
    // runs this target too (§8.3: the generated TypeScript is erasable syntax by design), so
    // the two declarations the projection adds have to be erasable as well.
    if have("node") {
        let script = format!(
            "import('./order_shipping.ts').then(m => console.log(String(m.order_shipping_from(JSON.parse({:?})))));",
            order_json()
        );
        check(
            "TypeScript",
            Command::new("node")
                .current_dir(dir.join("typescript"))
                .args(["--no-warnings", "--input-type=module", "-e", &script]),
        );
        ran += 1;
    }

    eprintln!("射影を走らせた言語: {ran}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A PHP single-quoted string for `php -r`.
fn php_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// The five that generate it, and only those five: a target that grew one without anyone
/// deciding to is a target whose caller's object this tool started naming (§15.125).
#[test]
fn 射影を出すのは五つの言語だけ() {
    let dir = generated("where");
    let has = |rel: &str, needle: &str| {
        std::fs::read_to_string(dir.join(rel)).map(|s| s.contains(needle)).unwrap_or(false)
    };
    for (rel, needle) in [
        ("python/order_shipping.py", "def order_shipping_from("),
        ("typescript/order_shipping.ts", "export function order_shipping_from("),
        ("javascript/order_shipping.mjs", "export function order_shipping_from("),
        ("ruby/order_shipping.rb", "def self.order_shipping_from("),
        ("php/order_shipping.php", "function order_shipping_from("),
    ] {
        assert!(has(rel, needle), "{rel} に射影関数がありません");
    }
    for rel in ["rust/order_shipping.rs", "go/ordershipping/order_shipping.go", "swift/order_shipping.swift", "java/OrderShipping.java", "sql/order_shipping.sql"] {
        assert!(!has(rel, "_from("), "{rel} に射影関数が出ています");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The paths are held to the contract, and the contract is read from beside the rule.
#[test]
fn 契約のフィールドが変われば止まる() {
    let dir = std::env::temp_dir().join(format!("rulec-projection-moved-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("contracts")).unwrap();
    let rule = std::fs::read_to_string(root().join("tests/corpus/注文の送料.rule")).unwrap();
    let schema = std::fs::read_to_string(root().join("tests/corpus/contracts/order.schema.json")).unwrap();
    std::fs::write(dir.join("r.rule"), &rule).unwrap();
    // The contract renames one field, which is the change nothing else in this tool can see.
    std::fs::write(dir.join("contracts/order.schema.json"), schema.replace("\"zone\"", "\"region\"")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .args(["check", &dir.join("r.rule").to_string_lossy(), "--format", "json"])
        .output()
        .expect("rulec を起動できない");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("\"E121\""), "フィールドの名前が変わっても止まらない:\n{out}");
    assert_eq!(o.status.code(), Some(1), "E121 は error です");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rule that projects nothing gets no projection function and no `_Obj` type: the five
/// emitters ask before writing anything.
#[test]
fn 射影の無い規則には何も出ない() {
    let dir = std::env::temp_dir().join(format!("rulec-projection-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", "tests/corpus/送料.rule", "--out", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success());
    let names = |d: &Path| -> Vec<String> {
        std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
    };
    for lang in ["python", "typescript", "javascript", "ruby", "php"] {
        for n in names(&dir.join(lang)) {
            let Ok(s) = std::fs::read_to_string(dir.join(lang).join(&n)) else { continue };
            assert!(!s.contains("_from("), "{lang}/{n} に射影関数が出ています");
            assert!(!s.contains("_Obj"), "{lang}/{n} に射影の型が出ています");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A projected **date** becomes the day number the rule counts in, which means each of the
/// five carries its own `days_from_civil` — five hand-written copies of one formula, and no
/// corpus rule projects a date, so nothing ran any of them.
///
/// So a sweep: one date every 37 days across eleven years, leap days included, through every
/// language, held to what `datetime` says. Agreeing on the boundary is the whole claim: a
/// copy that is off by a day, or that reads February wrong, moves it.
#[test]
fn 日付の射影は五つの言語で同じ日を指す() {
    let dir = std::env::temp_dir().join(format!("rulec-projection-date-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("o.json"),
        "{\"$defs\":{\"O\":{\"type\":\"object\",\"properties\":{\"placed_at\":{\"type\":\"string\"}},\"required\":[\"placed_at\"]}}}\n",
    )
    .unwrap();
    let rule = "rule d(d) v1\n\n\
        shape o(o) = jsonschema \"o.json\" \"#/$defs/O\"\n\n\
        inputs\n  placed(placed) : date  range >=2020-01-01 <=2030-12-31  from o.placed_at\n\n\
        outputs\n  first_half(first_half) : bool\n\n\
        table decide(decide)\npolicy unique\n\
        | placed       | -> first_half(first_half) : bool |\n\
        | <=2026-06-30 | true                             |\n\
        | >2026-06-30  | false                            |\n";
    std::fs::write(dir.join("d.rule"), rule).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .args(["gen", &dir.join("d.rule").to_string_lossy(), "--out", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));

    // One date every 37 days, and what each one's answer has to be.
    let dates = sweep();
    let want: Vec<bool> = dates.iter().map(|d| d.as_str() <= "2026-06-30").collect();
    let lines: Vec<String> = dates.iter().map(|d| format!("{{\"placed_at\":\"{d}\"}}")).collect();
    let payload = lines.join("\n");
    let mut ran = 0;

    let say = |lang: &str, out: &str, want: &[bool], dates: &[String]| {
        let got: Vec<bool> = out.trim().split(',').map(|x| x == "true").collect();
        assert_eq!(got.len(), want.len(), "{lang}: 件数が違う: {out}");
        for (k, (g, w)) in got.iter().zip(want).enumerate() {
            assert_eq!(g, w, "{lang}: {} の答えが違う", dates[k]);
        }
    };

    assert!(have("python3"), "python3 が要ります");
    let script = format!(
        "import json,sys,d\nprint(','.join('true' if d.d_from(json.loads(l)) else 'false' for l in sys.stdin if l.strip()))\n"
    );
    let out = piped("Python", Command::new("python3").current_dir(dir.join("python")).args(["-B", "-c", &script]), &payload);
    say("Python", &out, &want, &dates);
    ran += 1;

    for (lang, sub, file) in
        [("JavaScript", "javascript", "d.mjs"), ("TypeScript", "typescript", "d.ts")]
    {
        if !have("node") {
            eprintln!("注意: node が無いので {lang} 側を飛ばした");
            continue;
        }
        let script = format!(
            "const m=await import('./{file}');const o=[];for(const l of `{payload}`.split('\\n'))if(l.trim())o.push(m.d_from(JSON.parse(l))?'true':'false');console.log(o.join(','));"
        );
        let out = piped(
            lang,
            Command::new("node").current_dir(dir.join(sub)).args(["--no-warnings", "--input-type=module", "-e", &script]),
            "",
        );
        say(lang, &out, &want, &dates);
        ran += 1;
    }

    if have("ruby") {
        let script = "require 'json'\nrequire './d.rb'\nputs STDIN.read.split(\"\\n\").reject(&:empty?).map{|l| D.d_from(JSON.parse(l))}.join(',')";
        let out = piped("Ruby", Command::new("ruby").current_dir(dir.join("ruby")).args(["-e", script]), &payload);
        say("Ruby", &out, &want, &dates);
        ran += 1;
    } else {
        eprintln!("注意: ruby が無いので Ruby 側を飛ばした");
    }

    if have("php") {
        let script = "require './d.php';\n$o=[];foreach(explode(\"\\n\",trim(stream_get_contents(STDIN))) as $l){ if(trim($l)==='')continue; $o[]=\\D\\d_from(json_decode($l,true))?'true':'false'; }\necho implode(',',$o);";
        let out = piped("PHP", Command::new("php").current_dir(dir.join("php")).args(["-r", script]), &payload);
        say("PHP", &out, &want, &dates);
        ran += 1;
    } else {
        eprintln!("注意: php が無いので PHP 側を飛ばした");
    }

    eprintln!("日付の射影を走らせた言語: {ran}（日付 {} 件）", dates.len());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The first and last day of every month in the declared range, both kinds of February
/// included, **and the two days either side of the rule's own boundary** — which is the pair
/// that actually pins the conversion. A copy that is off by a constant moves the boundary and
/// nothing else, so a sweep that steps over it proves nothing.
fn sweep() -> Vec<String> {
    let leap = |y: i32| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let len = |y: i32, m: u32| match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if leap(y) {
                29
            } else {
                28
            }
        }
    };
    let mut out = vec!["2026-06-30".to_string(), "2026-07-01".to_string()];
    for y in 2020..=2030 {
        for m in 1..=12u32 {
            out.push(format!("{y:04}-{m:02}-01"));
            out.push(format!("{y:04}-{m:02}-{:02}", len(y, m)));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Run a command with `input` on its stdin and give back what it printed.
fn piped(lang: &str, cmd: &mut Command, input: &str) -> String {
    use std::io::Write;
    let mut p = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("起動できない");
    p.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    assert!(o.status.success(), "{lang}: 走らない:\n{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// An optional input takes a field the contract lets an object leave out, and a missing one is
/// read as none — whether the field itself is missing or the object on the way to it is
/// (§15.132). Before, all five indexed straight into the object and failed on a missing key,
/// which left no way to take such a field: the check now refuses a required input read from
/// one, and the answer it points to is `T?`.
#[test]
fn 省略できる入力は無いフィールドを_none_として読む() {
    const RULE: &str = "rule 任意の割引(opt_discount) v1\n\n\
        shape 注文(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
        enum 種別(kind) = percent(percent) | fixed(fixed)\n\n\
        inputs\n  種別(kind) : 種別?  from 注文.coupon.kind\n\n\
        outputs\n  割引(off) : money[円]  round down(1円)\n\n\
        table 割引表(t)\npolicy unique\n| 種別    | -> 割引 |\n| none    | 0円     |\n| percent | 100円   |\n| fixed   | 200円   |\n";
    const SCHEMA: &str = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"properties\":{\"coupon\":{\"$ref\":\"#/$defs/Coupon\"}}},\
        \"Coupon\":{\"type\":\"object\",\"properties\":{\"kind\":{\"type\":\"string\",\"enum\":[\"percent\",\"fixed\"]}}}}}";
    let dir = std::env::temp_dir().join(format!("rulec-projection-opt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("r.rule"), RULE).unwrap();
    std::fs::write(dir.join("order.json"), SCHEMA).unwrap();
    let rulec = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(&dir).args(args).output().expect("rulec を起動できない");
    let o = rulec(&["check", "r.rule"]);
    assert!(o.status.success(), "省略できる入力に `required` を求めてはいけない:\n{}", String::from_utf8_lossy(&o.stdout));
    let o = rulec(&["gen", "r.rule", "--out", "gen"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let g = dir.join("gen");
    let cases = [("{}", "0"), ("{\"coupon\":{}}", "0"), ("{\"coupon\":{\"kind\":null}}", "0"), ("{\"coupon\":{\"kind\":\"fixed\"}}", "200")];
    let want = |lang: &str, cmd: &mut Command, want: &str, case: &str| {
        let o = cmd.output().unwrap_or_else(|e| panic!("{lang} を起動できない: {e}"));
        let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
        assert!(o.status.success(), "{lang}: {case} で落ちた:\n{out}\n{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(out, want, "{lang}: {case} の答えが違う");
    };
    for (case, w) in cases {
        let py = format!("import json, opt_discount as m\nprint(m.opt_discount_from(json.loads({case:?})))\n");
        want("Python", Command::new("python3").current_dir(g.join("python")).args(["-B", "-c", &py]), w, case);
        if have("node") {
            for (lang, sub, file, extra) in [("JavaScript", "javascript", "opt_discount.mjs", None), ("TypeScript", "typescript", "opt_discount.ts", Some("--no-warnings"))] {
                let js = format!("import('./{file}').then(m => console.log(String(m.opt_discount_from(JSON.parse({case:?})))));");
                let mut cmd = Command::new("node");
                cmd.current_dir(g.join(sub));
                if let Some(x) = extra {
                    cmd.arg(x);
                }
                want(lang, cmd.args(["--input-type=module", "-e", &js]), w, case);
            }
        }
        if have("ruby") {
            let rb = format!("require 'json'\nrequire './opt_discount.rb'\nputs OptDiscount.opt_discount_from(JSON.parse({case:?}))\n");
            want("Ruby", Command::new("ruby").current_dir(g.join("ruby")).args(["-e", &rb]), w, case);
        }
        if have("php") {
            let php = format!("require './opt_discount.php'; echo \\OptDiscount\\opt_discount_from(json_decode({}, true));", php_str(case));
            want("PHP", Command::new("php").current_dir(g.join("php")).args(["-r", &php]), w, case);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rule and its contract written to a fresh directory, checked and generated: the directory
/// the five modules are in.
fn built(tag: &str, rule: &str, contract: (&str, &str)) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-projection-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("r.rule"), rule).unwrap();
    std::fs::write(dir.join(contract.0), contract.1).unwrap();
    let rulec = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(&dir).args(args).output().expect("rulec を起動できない");
    let o = rulec(&["check", "r.rule", "--format", "json"]);
    assert!(o.status.success(), "契約と規則はそろっているはず:\n{}", String::from_utf8_lossy(&o.stdout));
    let o = rulec(&["gen", "r.rule", "--out", "gen"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    dir
}

/// Each case — an object as JSON text, and the answer it has to get — through the projection
/// function of every one of the five whose toolchain is here. Python is required.
fn five(dir: &Path, alias: &str, cases: &[(String, &str)]) {
    let module: String = alias.split('_').map(|w| w[..1].to_uppercase() + &w[1..]).collect();
    let g = dir.join("gen");
    let want = |lang: &str, cmd: &mut Command, want: &str, case: &str| {
        let o = cmd.output().unwrap_or_else(|e| panic!("{lang} を起動できない: {e}"));
        let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
        assert!(o.status.success(), "{lang}: {case} で落ちた:\n{out}\n{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(out, want, "{lang}: {case} の答えが違う");
    };
    assert!(have("python3"), "python3 が要ります");
    for (case, w) in cases {
        let py = format!("import json, {alias} as m\nprint(m.{alias}_from(json.loads({case:?})))\n");
        want("Python", Command::new("python3").current_dir(g.join("python")).args(["-B", "-c", &py]), w, case);
        if have("node") {
            for (lang, sub, file) in [("JavaScript", "javascript", format!("{alias}.mjs")), ("TypeScript", "typescript", format!("{alias}.ts"))] {
                let js = format!("import('./{file}').then(m => console.log(String(m.{alias}_from(JSON.parse({case:?})))));");
                want(lang, Command::new("node").current_dir(g.join(sub)).args(["--no-warnings", "--input-type=module", "-e", &js]), w, case);
            }
        }
        if have("ruby") {
            let rb = format!("require 'json'\nrequire './{alias}.rb'\nputs {module}.{alias}_from(JSON.parse({case:?}))\n");
            want("Ruby", Command::new("ruby").current_dir(g.join("ruby")).args(["-e", &rb]), w, case);
        }
        if have("php") {
            let php = format!("require './{alias}.php'; echo \\{module}\\{alias}_from(json_decode({}, true));", php_str(case));
            want("PHP", Command::new("php").current_dir(g.join("php")).args(["-r", &php]), w, case);
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// A `.proto` shape is read in the JSON form protojson gives it (§15.133): a field under its
/// JSON name (`json_name`, or the lowerCamelCase of its name) or under its name as the
/// `.proto` writes it, an int64 as a string, and a field left out at its default, an empty
/// `repeated`, an unset `optional`, all read as proto reads them. Before, the five read the
/// `.proto` names straight and failed on every field protojson leaves out.
#[test]
fn protoの形はprotojsonのとおりに読む() {
    const PROTO: &str = "syntax = \"proto3\";\npackage shop.v1;\n\n\
        message Order {\n  Shipping shipping = 1 [(buf.validate.field).required = true];\n  \
        repeated Line order_lines = 2 [(buf.validate.field).repeated.max_items = 50];\n  \
        int64 total_jpy = 3 [json_name = \"total\", (buf.validate.field).int64 = {gte: 0, lte: 10000000}];\n  \
        optional string coupon_kind = 4 [(buf.validate.field).string = {in: [\"percent\", \"fixed\"]}];\n}\n\n\
        message Shipping {\n  string zone_code = 1 [(buf.validate.field).string = {in: [\"honshu\", \"hokkaido\", \"okinawa\"]}];\n}\n\n\
        message Line {\n  bool is_chilled = 1;\n  int64 amount = 2;\n}\n";
    const RULE: &str = "rule 注文の送料pb(order_fee_pb) v1\n\n\
        shape 注文(order) = proto \"order.proto\" shop.v1.Order\n\n\
        enum 地域(zone) = honshu(honshu) | hokkaido(hokkaido) | okinawa(okinawa)\n\
        enum 種別(kind) = percent(percent) | fixed(fixed)\n\n\
        inputs\n  \
        地域(zone)     : 地域                               from 注文.shipping.zone_code\n  \
        冷蔵あり(cold) : bool                               from any 注文.order_lines where is_chilled = true\n  \
        明細数(lines)  : number     range >=0 <=50          from count 注文.order_lines\n  \
        金額(total)    : money[円]  range >=0円 <=1000万円  from 注文.total_jpy\n  \
        種別(kind)     : 種別?                              from 注文.coupon_kind\n\n\
        outputs\n  送料(fee) : money[円]  round up(10円)\n\n\
        table 送料表(t)\npolicy first\n\
        | 地域    | 冷蔵あり | 明細数 | 金額      | 種別  | -> 送料 |\n\
        | -       | -        | -      | >=10000円 | -     | 0円     |\n\
        | okinawa | -        | -      | -         | -     | 1500円  |\n\
        | -       | true     | -      | -         | -     | 1300円  |\n\
        | -       | -        | >10    | -         | -     | 1000円  |\n\
        | -       | -        | -      | -         | fixed | 700円   |\n\
        | -       | -        | -      | -         | -     | 800円   |\n";
    let dir = built("pb", RULE, ("order.proto", PROTO));
    let eleven = vec!["{}"; 11].join(",");
    five(
        &dir,
        "order_fee_pb",
        &[
            // protojson as it comes by default: lowerCamelCase, defaults left out, int64 a string.
            ("{\"shipping\":{\"zoneCode\":\"honshu\"},\"orderLines\":[{\"isChilled\":true,\"amount\":\"100\"}],\"total\":\"5000\"}".into(), "1300"),
            // The names as the .proto writes them, and the defaults written out.
            ("{\"shipping\":{\"zone_code\":\"honshu\"},\"order_lines\":[{\"is_chilled\":false,\"amount\":\"100\"}],\"total_jpy\":\"0\",\"coupon_kind\":\"fixed\"}".into(), "700"),
            // Everything at its default: no lines, no total, no coupon.
            ("{\"shipping\":{\"zoneCode\":\"okinawa\"}}".into(), "1500"),
            // Eleven lines, none chilled: a `false` is left out of every element.
            (format!("{{\"shipping\":{{\"zoneCode\":\"hokkaido\"}},\"orderLines\":[{eleven}]}}"), "1000"),
            // An encoder that writes an int64 as a number.
            ("{\"shipping\":{\"zoneCode\":\"honshu\"},\"total\":20000}".into(), "0"),
        ],
    );
}

/// What a `where` tests on an element of a `.proto` shape is the value protojson carries
/// (§15.133): an int64 as a string, compared as the number it is, and an enum as its value's
/// name, which is left out when it is the value numbered 0. A where-test on an int64 compared
/// a string with a number before — a `TypeError` in Python, and never equal anywhere — and one
/// on an enum was not read at all.
#[test]
fn protoの要素のwhereはprotojsonの値で比べる() {
    const PROTO: &str = "syntax = \"proto3\";\npackage shop.v1;\n\n\
        enum Temp {\n  TEMP_AMBIENT = 0;\n  TEMP_CHILLED = 1;\n  TEMP_FROZEN = 2;\n}\n\n\
        message Order {\n  repeated Line lines = 1 [(buf.validate.field).repeated.max_items = 20];\n}\n\n\
        message Line {\n  Temp temp = 1;\n  int64 amount_jpy = 2;\n}\n";
    const RULE: &str = "rule 明細の判定(line_checks) v1\n\n\
        shape 注文(order) = proto \"order.proto\" shop.v1.Order\n\n\
        inputs\n  \
        冷凍あり(frozen)  : bool                     from any 注文.lines where temp = TEMP_FROZEN\n  \
        常温だけ(ambient) : bool                     from all 注文.lines where temp = TEMP_AMBIENT\n  \
        高額の数(big)     : number  range >=0 <=20  from count 注文.lines where amount_jpy >= 10000\n  \
        ちょうど(exact)   : bool                     from any 注文.lines where amount_jpy = 5000\n\n\
        outputs\n  送料(fee) : money[円]  round up(10円)\n\n\
        table 送料表(t)\npolicy first\n\
        | 冷凍あり | 常温だけ | 高額の数 | ちょうど | -> 送料 |\n\
        | true     | -        | -        | -        | 1500円  |\n\
        | -        | -        | >=2      | -        | 900円   |\n\
        | -        | true     | -        | true     | 600円   |\n\
        | -        | true     | -        | -        | 500円   |\n\
        | -        | -        | -        | true     | 700円   |\n\
        | -        | -        | -        | -        | 800円   |\n";
    let dir = built("pb-where", RULE, ("order.proto", PROTO));
    five(
        &dir,
        "line_checks",
        &[
            ("{\"lines\":[{\"temp\":\"TEMP_FROZEN\",\"amountJpy\":\"100\"}]}".into(), "1500"),
            // Neither line says `temp`, so both are TEMP_AMBIENT; the second is 5000 as a string.
            ("{\"lines\":[{},{\"amountJpy\":\"5000\"}]}".into(), "600"),
            // Two lines at 10000 or more, the second under the name the .proto writes.
            ("{\"lines\":[{\"temp\":\"TEMP_CHILLED\",\"amountJpy\":\"20000\"},{\"amount_jpy\":\"10000\"}]}".into(), "900"),
            // No lines: `all` holds of none, `any` of none does not.
            ("{}".into(), "500"),
            // An encoder that writes an int64 as a number.
            ("{\"lines\":[{\"temp\":\"TEMP_CHILLED\",\"amountJpy\":5000}]}".into(), "700"),
        ],
    );
}

/// A date in a `where` is compared with the string the contract carries. It was written into
/// the five as `2026-01-01` bare — arithmetic in four of them, and a syntax error in Python,
/// whose integers take no leading zero.
#[test]
fn whereの日付は文字列として比べる() {
    const SCHEMA: &str = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"required\":[\"lines\"],\"properties\":{\"lines\":{\"type\":\"array\",\"maxItems\":20,\
        \"items\":{\"type\":\"object\",\"required\":[\"ships_on\"],\"properties\":{\"ships_on\":{\"type\":\"string\",\"format\":\"date\"}}}}}}}}";
    const RULE: &str = "rule 出荷日の判定(ship_dates) v1\n\n\
        shape 注文(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
        inputs\n  \
        今年(this_year)   : bool  from any 注文.lines where ships_on >= 2026-01-01\n  \
        七月一日(july)    : bool  from any 注文.lines where ships_on = 2026-07-01\n\n\
        outputs\n  送料(fee) : money[円]  round up(10円)\n\n\
        table 送料表(t)\npolicy first\n\
        | 今年  | 七月一日 | -> 送料 |\n\
        | -     | true     | 0円     |\n\
        | true  | -        | 500円   |\n\
        | false | -        | 800円   |\n";
    let dir = built("where-date", RULE, ("order.json", SCHEMA));
    five(
        &dir,
        "ship_dates",
        &[
            ("{\"lines\":[{\"ships_on\":\"2025-12-31\"}]}".into(), "800"),
            ("{\"lines\":[{\"ships_on\":\"2025-12-31\"},{\"ships_on\":\"2026-01-01\"}]}".into(), "500"),
            ("{\"lines\":[{\"ships_on\":\"2026-07-01\"}]}".into(), "0"),
        ],
    );
}

/// The corpus rule the examples page shows for a `.proto` contract, called the way its caller
/// would call it: with a request in protojson's form — names in lowerCamelCase, the int64 a
/// string, what is at its default left out — and once with the `.proto`'s own names. Each
/// answer is one of the rule's own examples, so what this holds the five to is a number the
/// reference evaluator already agreed with.
#[test]
fn コーパスのprotoの規則は要求をそのまま読む() {
    let dir = std::env::temp_dir().join(format!("rulec-projection-shipment-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", "tests/corpus/出荷の送料.rule", "--out", &dir.join("gen").to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let parcel = |fragile: bool| {
        if fragile { "{\"weightG\":\"1200\",\"handling\":\"HANDLING_FRAGILE\"}" } else { "{\"weightG\":\"1200\"}" }
    };
    let many = |n: usize, fragile: bool| vec![parcel(fragile); n].join(",");
    five(
        &dir,
        "shipment_fee",
        &[
            // honshu | false | 1 | 0円 | none -> 800円: the value and the window left out.
            (format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{}]}}", many(1, false)), "800"),
            // hokkaido | true | 2 | 5万円 | none -> 2900円
            (
                format!("{{\"destination\":{{\"region\":\"hokkaido\"}},\"parcels\":[{},{}],\"declaredValueJpy\":\"50000\"}}", parcel(true), parcel(false)),
                "2900",
            ),
            // okinawa | false | 3 | 20万円 | evening -> 5100円
            (
                format!("{{\"destination\":{{\"region\":\"okinawa\"}},\"parcels\":[{}],\"declaredValueJpy\":\"200000\",\"deliveryWindow\":\"evening\"}}", many(3, false)),
                "5100",
            ),
            // honshu | true | 20 | 100万円 | morning -> 16900円, under the names the .proto writes.
            (
                format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{}],\"declared_value_jpy\":\"1000000\",\"delivery_window\":\"morning\"}}", many(20, true)),
                "16900",
            ),
        ],
    );
}
