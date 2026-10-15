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
use ritsu_testkit::{Need, TempDir, need, ready};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// The corpus rule that projects, generated once.
fn generated(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("projection-{tag}"));
    let dir = t.path().to_path_buf();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(root())
        .args(["gen", "tests/corpus/注文の送料.rule", "--out", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    (t, dir)
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
    if !need(Need::Python) {
        return;
    }
    let (_tmp, dir) = generated("run");
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
    if ready(Need::Node, || have("node"), "node が無いので JavaScript 側を飛ばした") {
        let script = format!(
            "import('./order_shipping.mjs').then(m => console.log(String(m.order_shipping_from(JSON.parse({:?})))));",
            order_json()
        );
        check("JavaScript", Command::new("node").current_dir(dir.join("javascript")).args(["--input-type=module", "-e", &script]));
        ran += 1;
    }

    // --- Ruby
    if ready(Need::Ruby, || have("ruby"), "ruby が無いので Ruby 側を飛ばした") {
        let script = format!(
            "require 'json'\nrequire './order_shipping.rb'\nputs OrderShipping.order_shipping_from(JSON.parse({:?}))\n",
            order_json()
        );
        check("Ruby", Command::new("ruby").current_dir(dir.join("ruby")).args(["-e", &script]));
        ran += 1;
    }

    // --- PHP
    if ready(Need::Php, || have("php"), "php が無いので PHP 側を飛ばした") {
        let script = format!(
            "require './order_shipping.php'; echo \\OrderShipping\\order_shipping_from(json_decode({}, true));",
            php_str(&order_json())
        );
        check("PHP", Command::new("php").current_dir(dir.join("php")).args(["-r", &script]));
        ran += 1;
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
}

/// A PHP single-quoted string for `php -r`.
fn php_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// The five that generate it, and only those five: a target that grew one without anyone
/// deciding to is a target whose caller's object this tool started naming (§15.125).
#[test]
fn 射影を出すのは五つの言語だけ() {
    let (_tmp, dir) = generated("where");
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
}

/// The paths are held to the contract, and the contract is read from beside the rule.
#[test]
fn 契約のフィールドが変われば止まる() {
    let tmp = TempDir::new("projection-moved");
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(dir.join("contracts")).unwrap();
    let rule = std::fs::read_to_string(root().join("tests/corpus/注文の送料.rule")).unwrap();
    let schema = std::fs::read_to_string(root().join("tests/corpus/contracts/order.schema.json")).unwrap();
    std::fs::write(dir.join("r.rule"), &rule).unwrap();
    // The contract renames one field, which is the change nothing else in this tool can see.
    std::fs::write(dir.join("contracts/order.schema.json"), schema.replace("\"zone\"", "\"region\"")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .args(["check", &dir.join("r.rule").to_string_lossy(), "--format", "json"])
        .output()
        .expect("rulec を起動できない");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("\"E121\""), "フィールドの名前が変わっても止まらない:\n{out}");
    assert_eq!(o.status.code(), Some(1), "E121 は error です");
}

/// A `.proto` contract that does not read is named as a file that cannot be read, with where it
/// stops (§15.166), not as a contract that has nothing where the shape points.
#[test]
fn 読めない_proto_の契約は読めないと言う() {
    let tmp = TempDir::new("projection-unread");
    let dir = tmp.path().to_path_buf();
    std::fs::create_dir_all(dir.join("contracts")).unwrap();
    let rule = std::fs::read_to_string(root().join("tests/corpus/出荷の送料.rule")).unwrap();
    let proto = std::fs::read_to_string(root().join("tests/corpus/contracts/shipment.proto")).unwrap();
    std::fs::write(dir.join("r.rule"), &rule).unwrap();
    std::fs::write(dir.join("contracts/shipment.proto"), proto.trim_end().trim_end_matches('}')).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .args(["check", &dir.join("r.rule").to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{out}");
    assert!(out.contains("error[E013]: Cannot read `contracts/shipment.proto`"), "{out}");
    assert!(out.contains("It does not read as a `.proto` (line ") && out.contains("`}` is expected where the end of the file is"), "{out}");
    assert!(!out.contains("has nothing at"), "{out}");
}

/// A rule that projects nothing gets no projection function and no `_Obj` type: the five
/// emitters ask before writing anything.
#[test]
fn 射影の無い規則には何も出ない() {
    let tmp = TempDir::new("projection-none");
    let dir = tmp.path().to_path_buf();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
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
    if !need(Need::Python) {
        return;
    }
    let tmp = TempDir::new("projection-date");
    let dir = tmp.path().to_path_buf();
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
        .env("RULEC_LANG", "ja")
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
        if !ready(Need::Node, || have("node"), &format!("node が無いので {lang} 側を飛ばした")) {
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

    if ready(Need::Ruby, || have("ruby"), "ruby が無いので Ruby 側を飛ばした") {
        let script = "require 'json'\nrequire './d.rb'\nputs STDIN.read.split(\"\\n\").reject(&:empty?).map{|l| D.d_from(JSON.parse(l))}.join(',')";
        let out = piped("Ruby", Command::new("ruby").current_dir(dir.join("ruby")).args(["-e", script]), &payload);
        say("Ruby", &out, &want, &dates);
        ran += 1;
    }

    if ready(Need::Php, || have("php"), "php が無いので PHP 側を飛ばした") {
        let script = "require './d.php';\n$o=[];foreach(explode(\"\\n\",trim(stream_get_contents(STDIN))) as $l){ if(trim($l)==='')continue; $o[]=\\D\\d_from(json_decode($l,true))?'true':'false'; }\necho implode(',',$o);";
        let out = piped("PHP", Command::new("php").current_dir(dir.join("php")).args(["-r", script]), &payload);
        say("PHP", &out, &want, &dates);
        ran += 1;
    }

    eprintln!("日付の射影を走らせた言語: {ran}（日付 {} 件）", dates.len());
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
    if !need(Need::Python) {
        return;
    }
    const RULE: &str = "rule 任意の割引(opt_discount) v1\n\n\
        shape 注文(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
        enum 種別(kind) = percent(percent) | fixed(fixed)\n\n\
        inputs\n  種別(kind) : 種別?  from 注文.coupon.kind\n\n\
        outputs\n  割引(off) : money[円]  round down(1円)\n\n\
        table 割引表(t)\npolicy unique\n| 種別    | -> 割引 |\n| none    | 0円     |\n| percent | 100円   |\n| fixed   | 200円   |\n";
    const SCHEMA: &str = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"properties\":{\"coupon\":{\"$ref\":\"#/$defs/Coupon\"}}},\
        \"Coupon\":{\"type\":\"object\",\"properties\":{\"kind\":{\"type\":\"string\",\"enum\":[\"percent\",\"fixed\"]}}}}}";
    let tmp = TempDir::new("projection-opt");
    let dir = tmp.path().to_path_buf();
    std::fs::write(dir.join("r.rule"), RULE).unwrap();
    std::fs::write(dir.join("order.json"), SCHEMA).unwrap();
    let rulec = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").current_dir(&dir).args(args).output().expect("rulec を起動できない");
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
}

/// A rule and its contract written to a fresh directory, checked and generated: the directory
/// the five modules are in.
fn built(tag: &str, rule: &str, contract: (&str, &str)) -> (TempDir, PathBuf) {
    let tmp = TempDir::new(&format!("projection-{tag}"));
    let dir = tmp.path().to_path_buf();
    std::fs::write(dir.join("r.rule"), rule).unwrap();
    std::fs::write(dir.join(contract.0), contract.1).unwrap();
    let rulec = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").current_dir(&dir).args(args).output().expect("rulec を起動できない");
    let o = rulec(&["check", "r.rule", "--format", "json"]);
    assert!(o.status.success(), "契約と規則はそろっているはず:\n{}", String::from_utf8_lossy(&o.stdout));
    let o = rulec(&["gen", "r.rule", "--out", "gen"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    (tmp, dir)
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
    if !need(Need::Python) {
        return;
    }
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
    let (_tmp, dir) = built("pb", RULE, ("order.proto", PROTO));
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
    let (_tmp, dir) = built("pb-where", RULE, ("order.proto", PROTO));
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

/// A date in a `where` is compared with what the contract carries, read as the day it is: the
/// `YYYY-MM-DD` string, held to the calendar, against the day the cell names (§15.234). It was
/// written into the five as `2026-01-01` bare once — arithmetic in four of them, and a syntax
/// error in Python, whose integers take no leading zero — and then compared as strings, which
/// took a string that is no date for one.
#[test]
fn whereの日付は暦の日として比べる() {
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
    let (_tmp, dir) = built("where-date", RULE, ("order.json", SCHEMA));
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
    let tmp = TempDir::new("projection-shipment");
    let dir = tmp.path().to_path_buf();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
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

/// An order read straight from its contract (§15.223): the total and the largest fragile line
/// out of the object, and its lines as the sequence the rule walks. Each order is one of the
/// corpus rule's own examples, with the lines its `sequence` lists and the totals its row gives,
/// so what this holds the five to is a number the reference evaluator already agreed with. The
/// Japanese version of the rule is run the same way, under its own alias.
#[test]
fn 合計と最大と並びは五つの言語で同じ答えを出す() {
    let line = |q: u32, amount: u32, fragile: bool| format!("{{\"sku\":\"s{q}\",\"quantity\":{q},\"amount_usd\":{amount},\"fragile\":{fragile}}}");
    let order = |lines: &[String]| format!("{{\"id\":\"o1\",\"lines\":[{}]}}", lines.join(","));
    let cases: Vec<(String, &str)> = vec![
        // small_order: 120USD, nothing fragile, no line for a pallet.
        (order(&[line(2, 40, false), line(1, 80, false)]), "5"),
        // large_order: 12000USD, handled for free; three lines of 2000USD or more need the forklift.
        (order(&[line(3, 5000, false), line(2, 5000, false), line(1, 2000, false)]), "10"),
        // glass_order: the fragile line of 1500USD is the largest.
        (order(&[line(1, 1500, true), line(4, 600, false)]), "25"),
        // bulk_order: two lines of 50 pieces or more go on pallets, and need the forklift.
        (order(&[line(60, 300, false), line(80, 400, false), line(5, 200, false)]), "15"),
        // The fragile line is not the largest line: the `where` picks the fragile ones first.
        (order(&[line(1, 1200, false), line(2, 300, true)]), "5"),
    ];
    for (rule, alias) in [("tests/corpus/order_handling_fee.rule", "order_handling_fee"), ("tests/corpus/注文の取扱手数料.rule", "order_handling")] {
        let tmp = TempDir::new("projection-facts");
        let dir = tmp.path().to_path_buf();
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .env("RULEC_LANG", "ja")
            .current_dir(root())
            .args(["gen", rule, "--out", &dir.join("gen").to_string_lossy()])
            .output()
            .expect("rulec を起動できない");
        assert!(o.status.success(), "{rule}: {}", String::from_utf8_lossy(&o.stdout));
        five(&dir, alias, &cases);
    }
}

/// `sum`, `max` and `min` over a `.proto` collection, and a sequence taken from it, read in the
/// JSON form protojson gives (§15.223, §15.133): lowerCamelCase names or the `.proto`'s own, an
/// int64 as a string or a number, an enum's value numbered 0 and a `false` left out of an
/// element, and no collection at all. The latest date the rule reads is the earliest of the lines'.
#[test]
fn protoの合計と最大と最小と並びはprotojsonのとおりに読む() {
    const PROTO: &str = "syntax = \"proto3\";\npackage shop.v1;\n\n\
        enum Temp {\n  TEMP_AMBIENT = 0;\n  TEMP_CHILLED = 1;\n}\n\n\
        message Order {\n  repeated Line order_lines = 1 [(buf.validate.field).repeated.max_items = 20];\n}\n\n\
        message Line {\n  int64 amount_usd = 1 [(buf.validate.field).int64 = {gte: 0, lte: 5000}];\n  \
        int64 qty = 2 [(buf.validate.field).int64 = {gte: 1, lte: 99}];\n  bool is_fragile = 3;\n  Temp temp = 4;\n  \
        string ships_on = 5 [(buf.validate.field).string.min_len = 1];\n}\n";
    const RULE: &str = "rule 明細の集計(line_totals) v1\n\n\
        shape 注文(order) = proto \"order.proto\" shop.v1.Order\n\n\
        inputs\n  \
        合計(total)       : money[USD]  range >=0USD <=100000USD  from sum 注文.order_lines of amount_usd\n  \
        割れ物(fragile)   : money[USD]  range >=0USD <=5000USD    from max 注文.order_lines of amount_usd where is_fragile = true empty 0USD\n  \
        冷蔵の最小(cold)  : money[USD]  range >=0USD <=5000USD    from min 注文.order_lines of amount_usd where temp = TEMP_CHILLED empty 0USD\n  \
        最初の出荷(first) : date  range >=2020-01-01 <=2030-12-31  from min 注文.order_lines of ships_on empty 2030-12-31\n\n\
        elements 明細(lines) from 注文.order_lines\n  \
        数量(qty)           : number  range >=1 <=99  from qty\n  \
        割れ物か(breakable) : bool                    from is_fragile\n\n\
        outputs\n  手数料(fee) : money[USD]  round up(1USD)\n\n\
        table 大口(bulk)\npolicy unique\n\
        | 数量 | 割れ物か | -> 大口か(big) : bool |\n\
        | >=50 | -        | true                  |\n\
        | <50  | true     | true                  |\n\
        | <50  | false    | false                 |\n\n\
        count 大口数(bigs) over 明細 where 大口か  range >=0 <=20\n\n\
        table 決め(decide)\npolicy first\n\
        | 合計       | 割れ物    | 冷蔵の最小 | 最初の出荷   | 大口数 | -> 手数料 |\n\
        | >=10000USD | -         | -          | -            | -      | 0USD      |\n\
        | -          | >=1000USD | -          | -            | -      | 25USD     |\n\
        | -          | -         | >=1USD     | -            | -      | 20USD     |\n\
        | -          | -         | -          | <=2026-06-30 | -      | 18USD     |\n\
        | -          | -         | -          | -            | >=2    | 15USD     |\n\
        | -          | -         | -          | -            | -      | 5USD      |\n";
    let (_tmp, dir) = built("pb-facts", RULE, ("order.proto", PROTO));
    five(
        &dir,
        "line_totals",
        &[
            // protojson as it comes: the fragile line is the largest fragile one.
            ("{\"orderLines\":[{\"amountUsd\":\"1200\",\"qty\":\"2\",\"isFragile\":true,\"shipsOn\":\"2027-01-05\"},{\"amountUsd\":\"300\",\"qty\":\"1\",\"shipsOn\":\"2027-02-01\"}]}".into(), "25"),
            // The `.proto`'s own names: the smallest chilled line.
            ("{\"order_lines\":[{\"amount_usd\":\"100\",\"qty\":\"3\",\"temp\":\"TEMP_CHILLED\",\"ships_on\":\"2027-01-05\"},{\"amount_usd\":\"40\",\"qty\":\"1\",\"temp\":\"TEMP_CHILLED\",\"ships_on\":\"2027-03-01\"}]}".into(), "20"),
            // An int64 written as a number, and the earliest date before the half of the year.
            ("{\"orderLines\":[{\"amountUsd\":100,\"qty\":\"3\",\"shipsOn\":\"2026-05-01\"},{\"amountUsd\":\"40\",\"qty\":\"1\",\"shipsOn\":\"2027-03-01\"}]}".into(), "18"),
            // Two lines of 50 pieces or more, a `false` left out of each.
            ("{\"orderLines\":[{\"amountUsd\":\"100\",\"qty\":\"60\",\"shipsOn\":\"2027-05-01\"},{\"amountUsd\":\"40\",\"qty\":\"70\",\"shipsOn\":\"2027-03-01\"}]}".into(), "15"),
            // No collection at all: a total of 0, and every `empty` value.
            ("{}".into(), "5"),
            // A total of 12000USD.
            ("{\"orderLines\":[{\"amountUsd\":\"5000\",\"qty\":\"1\",\"shipsOn\":\"2027-05-01\"},{\"amountUsd\":\"5000\",\"qty\":\"1\",\"shipsOn\":\"2027-03-01\"},{\"amountUsd\":\"2000\",\"qty\":\"1\",\"shipsOn\":\"2027-03-01\"}]}".into(), "0"),
        ],
    );
}

/// A rule that takes one sequence out of the object and is still passed another: the one it
/// takes is built in the projection function, the other stays its parameter, and what `rulec
/// api` says the function takes is what each module declares (§15.223). Before, the signatures in
/// `api` listed no sequence at all, while the functions took every one.
#[test]
fn 並びを取る規則のapiの署名は生成物と同じ() {
    const SCHEMA: &str = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"required\":[\"lines\"],\"properties\":{\"lines\":{\"type\":\"array\",\"maxItems\":10,\
        \"items\":{\"type\":\"object\",\"required\":[\"qty\"],\"properties\":{\"qty\":{\"type\":\"integer\",\"minimum\":1,\"maximum\":9}}}}}}}}";
    const RULE: &str = "rule 二つの並び(two_lists) v1\n\n\
        shape 注文(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
        elements 明細(lines) from 注文.lines\n  数(qty) : number  range >=1 <=9  from qty\n\n\
        elements 支払い(payments)\n  額(amount) : money[円]  range >=1円 <=1000円\n\n\
        outputs\n  件数(n) : number  round down(1)\n\n\
        count 多い(many) over 明細 where 多いか  range >=0 <=10\n\
        sum 払った(paid) over 支払い of 額  range >=0円 <=10000円\n";
    // A rule that walks two sequences and decides from both: `n` is how many lines have 5 or more.
    let rule = RULE.replace(
        "count 多い(many)",
        "table 判定(judge)\npolicy unique\n| 数  | -> 多いか(big) : bool |\n| >=5 | true                  |\n| <5  | false                 |\n\ncount 多い(many)",
    ) + "\ntable 決め(decide)\npolicy unique\n| 多い | 払った | -> 件数 |\n| -    | -      | 1       |\n";
    let (_tmp, dir) = built("two-lists", &rule, ("order.json", SCHEMA));
    let api = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(&dir).args(["api", "r.rule"]).output().expect("rulec を起動できない");
    let api = String::from_utf8_lossy(&api.stdout).into_owned();
    let out = dir.join("gen");
    for (lang, file, needle) in [
        ("python", "python/two_lists.py", "def two_lists_from("),
        ("typescript", "typescript/two_lists.ts", "export function two_lists_from("),
        ("php", "php/two_lists.php", "function two_lists_from("),
    ] {
        let text = std::fs::read_to_string(out.join(file)).unwrap();
        let line = text.lines().find(|l| l.starts_with(needle)).unwrap_or_else(|| panic!("{file} に射影関数がありません"));
        // The signature as the module writes it, up to the body.
        let sig = line.trim_end_matches(" {").trim_end_matches(':').to_string();
        let sig = if lang == "python" { format!("{sig}:") } else { sig };
        assert!(api.contains(&format!("\"signature\":{}", rulec::json::quote(&sig))), "{lang}: api の署名が生成物と違います:\n{sig}\n{api}");
        assert!(sig.contains("payments") && !sig.contains("lines"), "{lang}: 渡される並びは支払いだけのはず: {sig}");
    }
    assert!(api.contains("\"elements\":[{\"name\":\"明細\",\"alias\":\"lines\",\"from\":\"注文.lines\",\"fields\":[{\"name\":\"数\",\"alias\":\"qty\",\"from\":\"qty\"}]}]"), "{api}");
}

/// The shapes `from sum`, `from max`, `from min` and `elements … from` take, and what is said
/// when one is wrong (§15.223): E074 for the shape, E120 for a type, E121 for a path.
#[test]
fn 合計と最大と並びの形の誤りを言う() {
    const SCHEMA: &str = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"required\":[\"lines\",\"id\"],\"properties\":{\"id\":{\"type\":\"string\"},\"lines\":{\"type\":\"array\",\"maxItems\":10,\
        \"items\":{\"type\":\"object\",\"required\":[\"amount\",\"sku\"],\"properties\":{\"amount\":{\"type\":\"integer\",\"minimum\":0,\"maximum\":100},\"sku\":{\"type\":\"string\"}}}}}}}}";
    let check = |tag: &str, body: &str| -> String {
        let tmp = TempDir::new(&format!("facts-{tag}"));
        let dir = tmp.path().to_path_buf();
        std::fs::write(dir.join("order.json"), SCHEMA).unwrap();
        let rule = format!("rule t(t) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\n{body}\n\noutputs\n  x(x) : bool\n\ntable t(t)\npolicy unique\n| a | -> x |\n| - | true |\n");
        std::fs::write(dir.join("r.rule"), rule).unwrap();
        let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(&dir).args(["check", "r.rule"]).output().expect("rulec を起動できない");
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let input = |from: &str| format!("inputs\n  a(a) : money[USD]  range >=0USD <=1000USD  from {from}");
    for (tag, body, want) in [
        ("max-empty", input("max order.lines of amount"), "error[E074]: `from max` has no value for when no element is looked at"),
        ("sum-of", input("sum order.lines"), "error[E074]: The shape of `from sum` is wrong"),
        ("count-of", "inputs\n  a(a) : number  range >=0 <=10  from count order.lines of amount".to_string(), "error[E074]: `from count` takes no `of`"),
        ("sum-empty", input("sum order.lines of amount empty 0USD"), "error[E074]: `from sum` takes no `empty`"),
        ("empty-range", input("min order.lines of amount empty 2000USD"), "error[E074]: The `empty` value `2000USD` is outside the range of a"),
        ("empty-type", input("max order.lines of amount empty 2026-01-01"), "error[E074]: The `empty` value `2026-01-01` is not a value of money[USD]"),
        ("sum-bool", "inputs\n  a(a) : bool  from sum order.lines of amount".to_string(), "error[E120]: `from sum` yields a number"),
        ("of-string", input("max order.lines of sku empty 0USD"), "error[E120]: The field `sku` of an element does not fit the input"),
        ("of-missing", input("sum order.lines of price"), "error[E121]: An element has no field `price`"),
        ("not-collection", input("sum order.id of amount"), "error[E120]: `order.id` is not a collection"),
    ] {
        let out = check(tag, &body);
        assert!(out.contains(want), "{tag}: `{want}` が出ません:\n{out}");
        assert_eq!(out.matches("error[").count(), 1, "{tag}: エラーは一つのはず:\n{out}");
    }
    // A sequence taken from the contract: its line, and a field that says no path.
    let walked = |head: &str, field: &str| {
        format!("{head}\n  {field}\n\ninputs\n  a(a) : number  range >=0 <=10  from count order.lines\n\ntable b(b)\npolicy unique\n| amount | -> big : bool |\n| >=50USD | true         |\n| <50USD  | false        |\n\ncount n(n) over items where big  range >=0 <=10")
    };
    for (tag, body, want) in [
        ("seq-junk", walked("elements items(items) from order.lines where amount = 1", "amount(amount) : money[USD]  range >=0USD <=100USD  from amount"), "error[E074]: The shape of `elements … from` is wrong"),
        ("seq-word", walked("elements items(items) of order.lines", "amount(amount) : money[USD]  range >=0USD <=100USD"), "error[E074]: The shape of `elements … from` is wrong"),
        ("field-walk", walked("elements items(items) from order.lines", "amount(amount) : money[USD]  range >=0USD <=100USD  from sum amount"), "error[E074]: The shape of a field's `from` is wrong"),
        ("field-none", walked("elements items(items) from order.lines", "amount(amount) : money[USD]  range >=0USD <=100USD"), "error[E074]: The field amount does not say where it stands in an element of `order.lines`"),
        ("field-alone", walked("elements items(items)", "amount(amount) : money[USD]  range >=0USD <=100USD  from amount"), "error[E074]: The sequence items is not taken from a contract"),
        ("seq-path", walked("elements items(items) from order.items", "amount(amount) : money[USD]  range >=0USD <=100USD  from amount"), "error[E121]: The contract has no path `order.items`"),
        ("field-path", walked("elements items(items) from order.lines", "amount(amount) : money[USD]  range >=0USD <=100USD  from price"), "error[E121]: The contract has no path `order.lines[].price`"),
        ("field-type", walked("elements items(items) from order.lines", "amount(amount) : money[USD]  range >=0USD <=100USD  from sku"), "error[E120]: The type at `order.lines[].sku` does not fit the field"),
    ] {
        let out = check(tag, &body);
        assert!(out.contains(want), "{tag}: `{want}` が出ません:\n{out}");
    }
}

/// The seven targets that get no projection function are named as the rest is written, each with
/// the reason, and `rulec api` lists the same under `projection.without` (§15.223): a target that
/// cannot write a function says so by name, the way a walk or a column of strings does.
#[test]
fn 射影の関数の無い生成先を名前で言う() {
    let tmp = TempDir::new("projection-without");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root())
        .args(["gen", "tests/corpus/order_shipping_fee.rule", "--out", &tmp.path().to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success());
    let out = String::from_utf8_lossy(&o.stdout);
    let said = |name: &str| out.contains(&format!("{name}: the function that reads the inputs out of the caller's object (`order_shipping_fee_from`) is not generated for this target: "));
    for name in ["NumPy", "Rust", "Go", "Swift", "Java", "SQL", "Wasm"] {
        assert!(said(name), "{name} が名前で言われていません:\n{out}");
    }
    for name in ["Python", "TypeScript", "JavaScript", "Ruby", "PHP"] {
        assert!(!said(name), "{name} は射影の関数を書くのに、書かないと言われました:\n{out}");
    }
    let api = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(root()).args(["api", "tests/corpus/order_shipping_fee.rule"]).output().expect("rulec を起動できない");
    let api = rulec::json::parse(String::from_utf8_lossy(&api.stdout).trim()).expect("api の出力が JSON でない");
    let without = api.get("projection").and_then(|p| p.get("without")).expect("projection.without が無い");
    let keys: Vec<String> = rulec::json::members(without).expect("without がオブジェクトでない").into_keys().map(str::to_string).collect();
    let mut keys = keys;
    keys.sort();
    assert_eq!(keys, vec!["go", "java", "numpy", "rust", "sql", "swift", "wasm"]);
    // A rule that projects nothing says nothing of it.
    let tmp = TempDir::new("projection-without-none");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root())
        .args(["gen", "tests/corpus/member_shipping_fee.rule", "--out", &tmp.path().to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(!String::from_utf8_lossy(&o.stdout).contains("caller's object"));
}

/// What each of the five says of one object passed to `<alias>_from`: the answer, or `REFUSED`
/// and the sentence of the module's `RuleInputError` (§15.234). Python is required; a language
/// whose toolchain is not here is left out and said so.
fn five_say(made: &Path, alias: &str, case: &str) -> Vec<(&'static str, String)> {
    let module: String = alias.split('_').map(|w| w[..1].to_uppercase() + &w[1..]).collect();
    let run = |lang: &str, cmd: &mut Command| -> String {
        let o = cmd.output().unwrap_or_else(|e| panic!("{lang} を起動できない: {e}"));
        assert!(o.status.success(), "{lang}: {case} で落ちた:\n{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let mut out = Vec::new();
    let py = format!(
        "import json, {alias} as m\ntry:\n    print(m.{alias}_from(json.loads({case:?})))\nexcept m.RuleInputError as e:\n    print('REFUSED ' + e.what)\n"
    );
    out.push(("Python", run("Python", Command::new("python3").current_dir(made.join("python")).args(["-B", "-c", &py]))));
    if have("node") {
        for (lang, sub, file) in [("JavaScript", "javascript", format!("{alias}.mjs")), ("TypeScript", "typescript", format!("{alias}.ts"))] {
            let js = format!(
                "import('./{file}').then(m => {{ try {{ console.log(String(m.{alias}_from(JSON.parse({case:?})))); }} catch (e) {{ if (e instanceof m.RuleInputError) {{ console.log('REFUSED ' + e.what); }} else {{ throw e; }} }} }});"
            );
            out.push((lang, run(lang, Command::new("node").current_dir(made.join(sub)).args(["--no-warnings", "--input-type=module", "-e", &js]))));
        }
    } else {
        eprintln!("SKIP JavaScript, TypeScript: node is not installed");
    }
    if have("ruby") {
        let rb = format!("require 'json'\nrequire './{alias}.rb'\nbegin\n  puts {module}.{alias}_from(JSON.parse({case:?}))\nrescue {module}::RuleInputError => e\n  puts 'REFUSED ' + e.what\nend\n");
        out.push(("Ruby", run("Ruby", Command::new("ruby").current_dir(made.join("ruby")).args(["-e", &rb]))));
    } else {
        eprintln!("SKIP Ruby: ruby is not installed");
    }
    if have("php") {
        let php = format!(
            "require './{alias}.php'; try {{ echo \\{module}\\{alias}_from(json_decode({}, true)), \"\\n\"; }} catch (\\{module}\\RuleInputError $e) {{ echo 'REFUSED ' . $e->what, \"\\n\"; }}",
            php_str(case)
        );
        out.push(("PHP", run("PHP", Command::new("php").current_dir(made.join("php")).args(["-d", "display_errors=stderr", "-r", &php]))));
    } else {
        eprintln!("SKIP PHP: php is not installed");
    }
    out
}

/// The rule at `rule`, generated in English or Japanese into `gen` under `dir`.
fn generated_in(lang: &str, dir: &Path, rule: &str) -> PathBuf {
    let made = dir.join("gen");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(root()).args(["gen", rule, "--out", &made.to_string_lossy()]).output().expect("rulec を起動できない");
    assert!(o.status.success(), "{rule}: {}", String::from_utf8_lossy(&o.stdout));
    made
}

/// Every value the projection function reads out of the caller's object is held to the kind the
/// rule takes it as, as the rule's own door holds an argument, and one that is not stops the five
/// alike: `RuleInputError`, with a sentence that names the value's path (§15.234). Before, Python
/// read `1.5`, `true` and `"7"` for an amount as 1, 1 and 7; TypeScript and JavaScript read `true`
/// and `"7"` as numbers; Ruby and PHP read `"abc"` as 0; and Python tested a line's `fragile` by
/// its truth. A value with a point that is whole is still that integer, as at the door (§15.205). The corpus rule of a total, a largest fragile line and the lines themselves,
/// in English and in its Japanese version.
#[test]
fn 読む値は型どおりに確かめ_五つの言語で同じ文で止まる() {
    if !need(Need::Python) {
        return;
    }
    assert!(have("python3"), "python3 が要ります");
    let line = |amount: &str, fragile: &str, qty: &str| format!("{{\"sku\":\"s\",\"quantity\":{qty},\"amount_usd\":{amount},\"fragile\":{fragile}}}");
    let order = |lines: &[String]| format!("{{\"id\":\"o1\",\"lines\":[{}]}}", lines.join(","));
    let ok = line("1500", "true", "1");
    for (lang, rule, alias, at, said) in [
        ("en", "tests/corpus/order_handling_fee.rule", "order_handling_fee", "order", ["is not an integer", "is not a boolean", "is missing", "is not a sequence", "is not an object"]),
        ("ja", "tests/corpus/注文の取扱手数料.rule", "order_handling", "注文", ["が整数ではありません", "が真偽ではありません", "がありません", "が並びではありません", "がオブジェクトではありません"]),
    ] {
        let tmp = TempDir::new(&format!("projection-kinds-{alias}"));
        let made = generated_in(lang, tmp.path(), rule);
        let [integer, boolean, missing, sequence, object] = said;
        let cases: Vec<(String, String)> = vec![
            // glass_order: the answer, and a whole number written with a point is that number.
            (order(&[ok.clone(), line("600", "false", "4")]), "25".to_string()),
            (order(&[line("1500.0", "true", "1.0"), line("600", "false", "4")]), "25".to_string()),
            // an amount that is no integer, in each of its three readings before
            (order(&[ok.clone(), line("1.5", "false", "4")]), format!("REFUSED {at}.lines[].amount_usd {integer}")),
            (order(&[ok.clone(), line("true", "false", "4")]), format!("REFUSED {at}.lines[].amount_usd {integer}")),
            (order(&[ok.clone(), line("\"7\"", "false", "4")]), format!("REFUSED {at}.lines[].amount_usd {integer}")),
            // a `where` reads a boolean, and a number is not one
            (order(&[ok.clone(), line("600", "1", "4")]), format!("REFUSED {at}.lines[].fragile {boolean}")),
            (order(&[ok.clone(), "{\"sku\":\"s\",\"quantity\":4,\"amount_usd\":600}".to_string()]), format!("REFUSED {at}.lines[].fragile {missing}")),
            // a field of an element the rule walks
            (order(&[ok.clone(), line("600", "false", "\"abc\"")]), format!("REFUSED {at}.lines[].quantity {integer}")),
            // the collection, and an element of it
            // (an empty object is not used: PHP reads `{}` and `[]` as the same empty array)
            ("{\"id\":\"o1\",\"lines\":{\"first\":1}}".to_string(), format!("REFUSED {at}.lines {sequence}")),
            ("{\"id\":\"o1\"}".to_string(), format!("REFUSED {at}.lines {missing}")),
            ("{\"id\":\"o1\",\"lines\":[5]}".to_string(), format!("REFUSED {at}.lines[] {object}")),
            // two wrong values: the one read first, the same in every language
            (order(&[line("1.5", "false", "1"), line("600", "\"x\"", "4")]), format!("REFUSED {at}.lines[].amount_usd {integer}")),
        ];
        for (case, want) in &cases {
            for (l, got) in five_say(&made, alias, case) {
                assert_eq!(&got, want, "{alias} ({l}): {case}");
            }
        }
    }
    // Along a collection the five read element by element: whether the element passes its
    // `where`, then the value of it that is compared. The first line passes and its amount is not
    // an integer; the second line's `fragile` is not a boolean, and is read after. (`built`
    // generates in Japanese, so the sentences are the Japanese ones.)
    const SCHEMA: &str = r#"{"$defs":{"Order":{"type":"object","required":["lines"],"properties":{"lines":{"type":"array","maxItems":20,"items":{"type":"object","required":["amount_usd","fragile","ships_on"],"properties":{"amount_usd":{"type":"integer","minimum":0,"maximum":5000},"fragile":{"type":"boolean"},"ships_on":{"type":"string","format":"date"}}}}}}}}"#;
    const RULE: &str = "rule top_fragile(top_fragile) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
        inputs\n  top(top)   : money[USD]  range >=0USD <=5000USD  from max order.lines of amount_usd where fragile = true empty 0USD\n  \
        late(late) : bool  from any order.lines where ships_on >= 2026-07-01\n\n\
        outputs\n  fee(fee) : money[USD]  round up(1USD)\n\n\
        table fee_of\npolicy first\n| top       | late  | -> fee |\n| >=1000USD | -     | 25USD  |\n| -         | true  | 10USD  |\n| -         | -     | 5USD   |\n";
    let (_tmp, dir) = built("where-order", RULE, ("order.json", SCHEMA));
    let row = |a: &str, f: &str, d: &str| format!("{{\"amount_usd\":{a},\"fragile\":{f},\"ships_on\":{d}}}");
    for (case, want) in [
        (format!("{{\"lines\":[{},{}]}}", row("1.5", "true", "\"2026-01-01\""), row("3", "\"x\"", "\"2026-01-01\"")), "REFUSED order.lines[].amount_usd が整数ではありません"),
        (format!("{{\"lines\":[{},{}]}}", row("2000", "true", "\"2026-01-01\""), row("3", "false", "\"2026-08-01\"")), "25"),
        (format!("{{\"lines\":[{}]}}", row("3", "false", "\"2026-08-01\"")), "10"),
        // a date is held to the calendar before it is compared, and `any` reads every element
        (format!("{{\"lines\":[{},{}]}}", row("3", "false", "\"2026-08-01\""), row("3", "false", "\"2026-02-30\"")), "REFUSED order.lines[].ships_on が日付ではありません"),
    ] {
        for (l, got) in five_say(&dir.join("gen"), "top_fragile", &case) {
            assert_eq!(got, want, "top_fragile ({l}): {case}");
        }
    }
}

/// A `.proto` contract is read in its JSON form, and the values are held the same way (§15.234):
/// an int64 is a number or the decimal string protojson writes, and nothing else; an enum's value
/// is one of the rule's; a place on the way is an object, or not there, or `null` (the default).
/// The corpus rule that reads a shipment request.
#[test]
fn protoの値も型どおりに確かめる() {
    if !need(Need::Python) {
        return;
    }
    let tmp = TempDir::new("projection-proto-kinds");
    let made = generated_in("en", tmp.path(), "tests/corpus/出荷の送料.rule");
    let parcel = "{\"weightG\":\"1200\"}";
    for (case, want) in [
        // honshu | false | 1 | 0円 | none -> 800円, the value left out
        (format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{parcel}]}}"), "800".to_string()),
        (format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{parcel}],\"declaredValueJpy\":\"5万\"}}"), "REFUSED 出荷.declared_value_jpy is not an integer".to_string()),
        (format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{parcel}],\"declaredValueJpy\":true}}"), "REFUSED 出荷.declared_value_jpy is not an integer".to_string()),
        (format!("{{\"destination\":{{\"region\":\"kyushu\"}},\"parcels\":[{parcel}]}}"), "REFUSED 出荷.destination.region is not a value of enum 地域".to_string()),
        (format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{{\"handling\":7}}]}}"), "REFUSED 出荷.parcels[].handling is not a string".to_string()),
        // a place on the way that is not an object, where protojson writes a message
        (format!("{{\"destination\":\"honshu\",\"parcels\":[{parcel}]}}"), "REFUSED 出荷.destination is not an object".to_string()),
        // `null` is protojson's default: here, no window
        (format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{parcel}],\"deliveryWindow\":null}}"), "800".to_string()),
        // the second enum the function reads, and a value of an enum that is no string
        (format!("{{\"destination\":{{\"region\":\"honshu\"}},\"parcels\":[{parcel}],\"deliveryWindow\":\"night\"}}"), "REFUSED 出荷.delivery_window is not a value of enum 時間帯".to_string()),
        (format!("{{\"destination\":{{\"region\":5}},\"parcels\":[{parcel}]}}"), "REFUSED 出荷.destination.region is not a value of enum 地域".to_string()),
    ] {
        for (l, got) in five_say(&made, "shipment_fee", &case) {
            assert_eq!(got, want, "shipment_fee ({l}): {case}");
        }
    }
}

/// A value of an enum is read by a function of that enum's type, named after it (§15.244): an enum
/// `level` by `_level_of`. Where the module already has the name, the reader takes one more `_`.
/// `_seq_of` is the helper that reads a collection, and here `_level_of` is the constant of the
/// group `level_of`, so the two enums of this rule are read by `_seq_of_` and `_level_of_`. With
/// either name written twice, the second definition would take the place of the first, and the
/// collection would be read as an enum, or the group tested as a function. The five read the
/// object alike, and refuse a value that is not a string, or not one of the enum's, with the same
/// sentence.
#[test]
fn 列挙を読む関数はほかの名前とぶつからない() {
    if !need(Need::Python) {
        return;
    }
    const SCHEMA: &str = r#"{"$defs":{"Order":{"type":"object","required":["kind","tier","lines"],"properties":{"kind":{"type":"string","enum":["single","many"]},"tier":{"type":"string","enum":["low","high"]},"lines":{"type":"array","maxItems":10,"items":{"type":"object"}}}}}}"#;
    const RULE: &str = "rule seq_reader(seq_reader) v1\n\nshape order(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
        enum seq(seq) = single(single) | many(many)\nenum level(level) = low(low) | high(high)\n\n\
        group level_of(level_of) = high\n\n\
        inputs\n  kind(kind)   : seq    from order.kind\n  tier(tier)   : level  from order.tier\n  \
        lines(lines) : number  range >=0 <=10  from count order.lines\n\n\
        outputs\n  fee(fee) : money[USD]  round up(1USD)\n\n\
        table fee_of(fee_of)\npolicy unique\n\
        | kind   | tier     | lines | -> fee |\n\
        | many   | level_of | -     | 30USD  |\n\
        | many   | low      | -     | 20USD  |\n\
        | single | -        | >=5   | 10USD  |\n\
        | single | -        | <5    | 5USD   |\n";
    let tmp = TempDir::new("projection-readers");
    let dir = tmp.path();
    std::fs::write(dir.join("r.rule"), RULE).unwrap();
    std::fs::write(dir.join("order.json"), SCHEMA).unwrap();
    let rulec = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(dir).args(args).output().expect("rulec を起動できない");
    let o = rulec(&["check", "r.rule"]);
    let said = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success() && !said.contains("warning"), "規則は警告なしに通るはず:\n{said}");
    let o = rulec(&["gen", "r.rule", "--out", "gen"]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let py = std::fs::read_to_string(dir.join("gen/python/seq_reader.py")).unwrap();
    for want in [
        "\ndef _seq_of(v: Any, at: str) -> list[Any]:\n",
        "\ndef _seq_of_(v: Any, what: str) -> Seq:\n",
        "\n_level_of = frozenset({Level.HIGH})\n",
        "\ndef _level_of_(v: Any, what: str) -> Level:\n",
    ] {
        assert!(py.contains(want), "{want:?} がありません:\n{py}");
    }
    let order = |kind: &str, tier: &str, n: usize| format!("{{\"kind\":{kind},\"tier\":{tier},\"lines\":[{}]}}", vec!["{\"sku\":\"s\"}"; n].join(","));
    for (case, want) in [
        (order("\"many\"", "\"high\"", 1), "30"),
        (order("\"many\"", "\"low\"", 1), "20"),
        (order("\"single\"", "\"low\"", 5), "10"),
        (order("\"single\"", "\"high\"", 0), "5"),
        (order("\"several\"", "\"low\"", 1), "REFUSED order.kind is not a value of enum seq"),
        (order("1", "\"low\"", 1), "REFUSED order.kind is not a value of enum seq"),
        (order("\"single\"", "\"mid\"", 1), "REFUSED order.tier is not a value of enum level"),
        (order("\"single\"", "true", 1), "REFUSED order.tier is not a value of enum level"),
    ] {
        for (l, got) in five_say(&dir.join("gen"), "seq_reader", &case) {
            assert_eq!(got, want, "seq_reader ({l}): {case}");
        }
    }
}

/// An enum the rule reads from an `enums` file is read by a function of that file's type, which
/// the module imports from the file's module beside it (§15.212, §15.244): no rule of the corpus
/// projects one. The enums file is the corpus's `shared/catalog.rule`.
#[test]
fn ほかのファイルの列挙もその型の関数で読む() {
    if !need(Need::Python) {
        return;
    }
    const SCHEMA: &str = r#"{"$defs":{"Item":{"type":"object","required":["category","zone"],"properties":{"category":{"type":"string","enum":["food","apparel","electronics","furniture"]},"zone":{"type":"string","enum":["local","regional","national","overseas"]}}}}}"#;
    const RULE: &str = "rule item_fee v1\n\nshape item = jsonschema \"item.json\" \"#/$defs/Item\"\n\n\
        import enum category from \"shared/catalog.rule\"\nimport enum zone from \"shared/catalog.rule\"\n\n\
        inputs\n  kind : category  from item.category\n  dest : zone      from item.zone\n\n\
        outputs\n  fee : money[USD]  round up(1USD)\n\n\
        table fee_of\npolicy unique\n\
        | kind    | dest       | -> fee |\n\
        | fragile | <=regional | 20USD  |\n\
        | fragile | >regional  | 40USD  |\n\
        | food    | -          | 5USD   |\n\
        | apparel | -          | 8USD   |\n";
    let tmp = TempDir::new("projection-foreign-enum");
    let dir = tmp.path();
    std::fs::create_dir_all(dir.join("shared")).unwrap();
    std::fs::copy(root().join("tests/corpus/shared/catalog.rule"), dir.join("shared/catalog.rule")).unwrap();
    std::fs::write(dir.join("item_fee.rule"), RULE).unwrap();
    std::fs::write(dir.join("item.json"), SCHEMA).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(dir)
        .args(["gen", "item_fee.rule", "shared/catalog.rule", "--out", "gen"])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let py = std::fs::read_to_string(dir.join("gen/python/item_fee.py")).unwrap();
    for want in [
        "\nfrom catalog import Category as Category, Zone as Zone, zone_rank\n",
        "\ndef _category_of(v: Any, what: str) -> Category:\n",
        "\ndef _zone_of(v: Any, what: str) -> Zone:\n",
    ] {
        assert!(py.contains(want), "{want:?} がありません:\n{py}");
    }
    for (case, want) in [
        ("{\"category\":\"electronics\",\"zone\":\"overseas\"}", "40"),
        ("{\"category\":\"furniture\",\"zone\":\"local\"}", "20"),
        ("{\"category\":\"food\",\"zone\":\"national\"}", "5"),
        ("{\"category\":\"toys\",\"zone\":\"local\"}", "REFUSED item.category is not a value of enum category"),
        ("{\"category\":\"food\",\"zone\":3}", "REFUSED item.zone is not a value of enum zone"),
    ] {
        for (l, got) in five_say(&dir.join("gen"), "item_fee", case) {
            assert_eq!(got, want, "item_fee ({l}): {case}");
        }
    }
}
