//! The rule as one Connect service (§15.112): the `.proto` it is called through, and the
//! conversion behind it.
//!
//! `rulec test` drives the service over every vector where the toolchain is installed, and
//! that is the claim that matters. What is checked here is the rest: that the contract is a
//! file the proto toolchain accepts, that the set it declares is the set the rule declares —
//! read back by rulec's own reader, which is how a rule cites someone else's `.proto` — and
//! that what `rulec api` says about the endpoint is what the generated code does.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn generate(tag: &str, rules: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-connect-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut args: Vec<&str> = vec!["gen"];
    args.extend_from_slice(rules);
    args.extend_from_slice(&["--out", dir.to_str().unwrap()]);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(&args)
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    dir
}

fn api(rule: &str) -> rulec::json::Json {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["api", rule])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    rulec::json::parse(&String::from_utf8_lossy(&o.stdout)).expect("JSON でない")
}

const RULE: &str = "tests/corpus/送料.rule";

/// The path is the package, which is what a buf module asks for, and `rulec api` says the
/// same thing as the file that was written.
#[test]
fn 出た場所と_api_が言う場所が同じ() {
    let dir = generate("where", &[RULE]);
    let a = api(RULE);
    let c = a.get("connect").expect("connect の項が無い");
    let proto = c.get("proto").and_then(|x| x.as_str()).expect("proto が無い").to_string();
    assert_eq!(proto, "proto/rulec/shipping_fee/v4/shipping_fee.proto");
    assert!(dir.join(&proto).is_file(), "{proto} が書かれていない");
    let body = std::fs::read_to_string(dir.join(&proto)).unwrap();
    // The path spells the package (buf's PACKAGE_DIRECTORY_MATCH), the service carries the
    // suffix buf's lint asks for, and the method says it has no side effects.
    assert!(body.contains("package rulec.shipping_fee.v4;"), "{body}");
    assert!(body.contains("service ShippingFeeService {"), "{body}");
    assert!(body.contains("option idempotency_level = NO_SIDE_EFFECTS;"), "{body}");
    assert_eq!(c.get("path").and_then(|x| x.as_str()), Some("/rulec.shipping_fee.v4.ShippingFeeService/Decide"));
    for k in ["module", "runner"] {
        let f = c.get("python").and_then(|p| p.get(k)).and_then(|x| x.as_str()).expect(k).to_string();
        assert!(dir.join("python").join(&f).is_file(), "python/{f} が書かれていない");
    }
    // The two applications the service is written as, and the two files that make the
    // directory a buf module.
    let svc = std::fs::read_to_string(dir.join("python/shipping_fee_service.py")).unwrap();
    assert!(svc.contains("\napp = application()"), "ASGI のアプリがありません:\n{svc}");
    assert!(svc.contains("\nwsgi_app = wsgi_application()"), "WSGI のアプリがありません:\n{svc}");
    assert_eq!(c.get("python").and_then(|p| p.get("asgi")).and_then(|x| x.as_str()), Some("app"));
    assert_eq!(c.get("python").and_then(|p| p.get("wsgi")).and_then(|x| x.as_str()), Some("wsgi_app"));
    for k in ["buf_yaml", "buf_gen_yaml"] {
        let f = c.get(k).and_then(|x| x.as_str()).expect(k).to_string();
        assert!(dir.join(&f).is_file(), "{f} が書かれていない");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The values in the generated `.proto` are the rule's own, read back by the reader rulec
/// uses on someone else's file (§15.59). The two conventions meet here: what rulec writes,
/// rulec can read.
#[test]
fn 書いた列挙を自分で読み返せる() {
    let dir = generate("roundtrip", &[RULE]);
    let body = std::fs::read_to_string(dir.join("proto/rulec/shipping_fee/v4/shipping_fee.proto")).unwrap();
    let es = rulec::proto::enums(&body);
    let member = es.iter().find(|e| e.name == "MemberKind").expect("MemberKind が無い");
    assert_eq!(member.aliases(), ["basic", "gold", "platinum"]);
    let pref = es.iter().find(|e| e.name == "Prefecture").expect("Prefecture が無い");
    assert_eq!(pref.aliases().len(), 47);
    assert_eq!(pref.aliases()[0], "hokkaido");
    // The zero value is proto3's "not set" and is not one of the rule's values.
    assert!(body.contains("PREFECTURE_UNSPECIFIED = 0;"), "{body}");
    assert_eq!(rulec::proto::package(&body).as_deref(), Some("rulec.shipping_fee.v4"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// An enum whose values belong to a contract outside the rule is imported, not copied: the
/// rule cites that file (§15.59) and the service speaks the contract's own type.
#[test]
fn 取り込んだ列挙は宣言し直さずに_import_する() {
    let d = std::env::temp_dir().join(format!("rulec-connect-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("order.proto"),
        "syntax = \"proto3\";\npackage shop.v1;\nenum MemberTier {\n  MEMBER_TIER_UNSPECIFIED = 0;\n  MEMBER_TIER_BASIC = 1;\n  MEMBER_TIER_GOLD = 2;\n}\n",
    )
    .unwrap();
    let rule = d.join("配送料金.rule");
    std::fs::write(
        &rule,
        "rule 配送料金(delivery_fee) v1\n\n\
         import proto \"order.proto\" MemberTier -> 会員区分\n\
         enum 会員区分(tier) = 一般(basic) | ゴールド(gold) default\n\n\
         inputs\n  会員(m) : 会員区分\n\n\
         outputs\n  送料(fee) : money[円, incl_tax]  round up(10円)\n\n\
         table 送料表(fee_table)\npolicy first\n\
         | 会員 | -> 送料(fee) : money[円, incl_tax] |\n\
         | 一般 | 800円 |\n| - | 400円 |\n",
    )
    .unwrap();
    let out = d.join("out");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .args(["gen", rule.to_str().unwrap(), "--out", out.to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let body = std::fs::read_to_string(out.join("proto/rulec/delivery_fee/v1/delivery_fee.proto")).unwrap();
    assert!(body.contains("import \"order.proto\";"), "{body}");
    // The field's type is the contract's, package and all — not a second enum meaning the same.
    assert!(body.contains("shop.v1.MemberTier m = 1;"), "{body}");
    assert!(!body.contains("enum MemberKind"), "取り込んだ列挙を宣言し直しています:\n{body}");
    // And the service reads its values from the module protoc writes for **that** file.
    let svc = std::fs::read_to_string(out.join("python/delivery_fee_service.py")).unwrap();
    assert!(svc.contains("import order_pb\n"), "{svc}");
    assert!(svc.contains("order_pb.MemberTier.GOLD: m.Tier.GOLD"), "{svc}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Every corpus rule's service and runner parse as Python. It costs a second and it is the
/// floor under everything the toolchain-gated tests cannot reach on a machine without them.
#[test]
fn 生成した_python_は全部構文として通る() {
    if !have("python3") {
        eprintln!("python3 が無いので飛ばします");
        return;
    }
    let rules: Vec<String> = std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let refs: Vec<&str> = rules.iter().map(|s| s.as_str()).collect();
    let dir = generate("syntax", &refs);
    let py = dir.join("python");
    let files: Vec<String> = std::fs::read_dir(&py)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with("_service.py") || n.ends_with("_connect_runner.py"))
        .collect();
    assert!(files.len() >= 40, "生成が少なすぎます: {}", files.len());
    let o = Command::new("python3")
        .current_dir(&py)
        .arg("-m")
        .arg("py_compile")
        .args(&files)
        .output()
        .expect("python3 を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the proto toolchain says about the file, where it is installed: `buf lint` finds
/// nothing in it and `buf build` compiles it, with no configuration written by hand — `gen`
/// wrote the module's own. Skipped, not failed, where buf is not installed.
#[test]
fn 生成した_proto_は_buf_が受け取る() {
    let rules = [
        RULE,
        "tests/corpus/クーポン一枚.rule",
        "tests/corpus/買物かごの送料.rule",
        "tests/corpus/期間区分.rule",
        "tests/corpus/parcel_rate.rule",
    ];
    let dir = generate("toolchain", &rules);
    let proto = dir.join("proto");
    if have("buf") {
        // No configuration is written here: `gen` wrote the module's own `buf.yaml`, and what
        // is being checked is that buf accepts the tree as rulec left it.
        assert!(proto.join("buf.yaml").is_file(), "buf.yaml が生成されていない");
        assert!(proto.join("buf.gen.yaml").is_file(), "buf.gen.yaml が生成されていない");
        let o = Command::new("buf").current_dir(&proto).arg("lint").output().expect("buf を起動できない");
        assert!(
            o.status.success(),
            "buf lint が文句を言っています:\n{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        // `buf build` reads the same module and says whether protoc would accept it at all.
        let o = Command::new("buf").current_dir(&proto).args(["build", "-o", "/dev/null"]).output().expect("buf を起動できない");
        assert!(o.status.success(), "buf build が断りました:\n{}", String::from_utf8_lossy(&o.stderr));
    } else {
        eprintln!("buf が無いので lint を飛ばします");
    }
    let files: Vec<String> = walk(&proto).iter().map(|p| p.strip_prefix(&proto).unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(files.len(), rules.len(), "規則ごとに一つのはずです: {files:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every `.proto` under a directory.
fn walk(d: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().is_some_and(|x| x == "proto") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// The adapter for a service that is already running (§10.1): what it leaves to fill in is
/// the other side's messages, and everything else — the handshake, the ids, the refusal —
/// is written.
#[test]
fn アダプタのテンプレートは呼び先を包む() {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["adapter", RULE, "--template", "connect-python"])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let t = String::from_utf8_lossy(&o.stdout).into_owned();
    for want in [
        "\"ok\": True",              // the handshake the protocol asks for
        "except ConnectError as e:", // a record they cannot answer leaves the denominator
        "\"err\": str(e)",
        "req[\"id\"]",
        "届け先",                    // the inputs, by the rule's own names
        "\"送料\": got",
    ] {
        assert!(t.contains(want), "テンプレートに `{want}` がありません:\n{t}");
    }
    if have("python3") {
        let d = std::env::temp_dir().join(format!("rulec-connect-adapter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("adapter.py"), &t).unwrap();
        let c = Command::new("python3")
            .current_dir(&d)
            .args(["-m", "py_compile", "adapter.py"])
            .output()
            .expect("python3 を起動できない");
        assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// The `.proto` takes four names of its own, and none of them is a keyword anywhere else, so
/// nothing but this would notice (§15.112). protoc refusing a file the writer of the rule
/// never asked for is a late and confusing way to find out.
#[test]
fn プロトが取る名前とぶつかる別名は_check_が言う() {
    let d = std::env::temp_dir().join(format!("rulec-connect-name-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let rule = d.join("demo.rule");
    std::fs::write(
        &rule,
        "rule demo(demo) v1\n\ninputs\n  重量(weight) : mass[g]  range >=1g <=100g\n\n\
         outputs\n  記録(trace) : money[円]  round up(1円)\n\n\
         table 表(t)\npolicy first\n| 重量 | -> 記録(trace) : money[円] |\n| <=50g | 100円 |\n| - | 200円 |\n",
    )
    .unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .args(["check", rule.to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    // A warning, not an error: a rule nobody serves is free to name an output `trace`.
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(out.contains("W121") && out.contains("Connect:"), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The two files that speak Connect, held to `mypy --strict` like every other generated
/// Python (§15.22) — here rather than in `tests/threeway.rs`, because only here are the
/// stubs and the runtime they import present. Errors inside protoc's own output and inside
/// the plugin's are not this repository's to answer for, so what is read is the lines about
/// the files rulec wrote.
#[test]
fn 生成した_python_は_mypy_strict_を通る() {
    if rulec_connect_ready().is_none() {
        eprintln!("buf・プラグイン・connectrpc・uvicorn・mypy のどれかが無いので飛ばします");
        return;
    }
    let dir = generate("mypy", &[RULE, "tests/corpus/期間区分.rule", "tests/corpus/買物かごの送料.rule"]);
    let py = dir.join("python");
    let o = Command::new("buf")
        .current_dir(&py)
        .args([
            "generate",
            "../proto",
            "--template",
            r#"{"version":"v2","plugins":[{"local":"protoc-gen-py","out":"stubs"},{"local":"protoc-gen-connectrpc","out":"stubs"}]}"#,
        ])
        .output()
        .expect("buf を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let files: Vec<String> = std::fs::read_dir(&py)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with("_service.py") || n.ends_with("_connect_runner.py"))
        .collect();
    let o = Command::new("mypy")
        .current_dir(&py)
        .args(["--strict", "--no-color-output"])
        .args(&files)
        .output()
        .expect("mypy を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    let ours: Vec<&str> = said
        .lines()
        .filter(|l| l.contains(": error:"))
        .filter(|l| files.iter().any(|f| l.starts_with(f.as_str())))
        .collect();
    assert!(ours.is_empty(), "生成した Python が mypy --strict を通りません:\n{}", ours.join("\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whether everything the Connect side needs is here: the compiler, the plugin, the runtime
/// and the type checker.
fn rulec_connect_ready() -> Option<()> {
    let importable = |m: &str| {
        Command::new("python3")
            .args(["-c", &format!("import {m}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    (have("buf")
        && have("mypy")
        && have("protoc-gen-py")
        && have("protoc-gen-connectrpc")
        && importable("connectrpc")
        // The runner starts the ASGI half under uvicorn, so the type checker has to see it.
        && importable("uvicorn"))
    .then_some(())
}
