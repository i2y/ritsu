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

/// The items of a JSON array, or none.
fn items(j: &rulec::json::Json) -> &[rulec::json::Json] {
    match j {
        rulec::json::Json::Arr(v) => v,
        _ => &[],
    }
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
    // The contract is imported from where its package puts it in the module, and it is there:
    // a copy of the file `check` read, so that the module builds as it stands (§15.160).
    assert!(body.contains("import \"shop/v1/order.proto\";"), "{body}");
    assert_eq!(
        std::fs::read_to_string(out.join("proto/shop/v1/order.proto")).ok(),
        std::fs::read_to_string(d.join("order.proto")).ok(),
        "契約が写されていない"
    );
    // The field's type is the contract's, package and all — not a second enum meaning the same.
    assert!(body.contains("optional shop.v1.MemberTier m = 1;"), "{body}");
    assert!(!body.contains("enum MemberKind"), "取り込んだ列挙を宣言し直しています:\n{body}");
    // And the service reads its values from the module the plugin writes for **that** file,
    // inside `stubs/` where the rule's own stub imports it from, by number.
    let svc = std::fs::read_to_string(out.join("python/delivery_fee_service.py")).unwrap();
    assert!(svc.contains("from stubs.shop.v1 import order_pb as shop_v1_order_pb\n"), "{svc}");
    assert!(svc.contains("shop_v1_order_pb.MemberTier(2): m.Tier.GOLD,  # MEMBER_TIER_GOLD"), "{svc}");
    // `api` finds the contract from the rule's directory, wherever it is run from.
    for cwd in [root(), d.clone()] {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .current_dir(&cwd)
            .args(["api", rule.to_str().unwrap()])
            .output()
            .expect("rulec を起動できない");
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let a = rulec::json::parse(&String::from_utf8_lossy(&o.stdout)).expect("JSON でない");
        let c = a.get("connect").expect("connect の項が無い");
        let f = &c.get("request_fields").map(items).expect("request_fields")[0];
        assert_eq!(f.get("type").and_then(|x| x.as_str()), Some("shop.v1.MemberTier"), "{}", cwd.display());
        assert_eq!(f.get("enum").and_then(|x| x.as_str()), Some("会員区分"));
        let e = &c.get("enums").map(items).expect("enums")[0];
        let contract = e.get("contract").expect("contract");
        assert_eq!(contract.get("file").and_then(|x| x.as_str()), Some("order.proto"));
        assert_eq!(contract.get("proto").and_then(|x| x.as_str()), Some("proto/shop/v1/order.proto"));
        assert_eq!(e.get("unset").and_then(|x| x.as_str()), Some("MEMBER_TIER_UNSPECIFIED"));
        let values: Vec<(String, String, i128)> = e
            .get("values")
            .map(items)
            .expect("values")
            .iter()
            .map(|v| {
                (
                    v.get("name").and_then(|x| x.as_str()).unwrap().to_string(),
                    v.get("alias").and_then(|x| x.as_str()).unwrap().to_string(),
                    v.get("number").and_then(|x| x.as_int()).unwrap(),
                )
            })
            .collect();
        assert_eq!(values, [("一般".into(), "MEMBER_TIER_BASIC".into(), 1), ("ゴールド".into(), "MEMBER_TIER_GOLD".into(), 2)]);
    }
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

/// The approver's page `doc` renders is the page `gen` writes beside the module, wherever the
/// command is run from. Both run the generated JavaScript, and a rule that reads its inputs
/// out of a `.proto` has that contract found from the rule's directory (§15.160): looked for
/// from the working directory instead, the page carried a reader that was not protojson's.
#[test]
fn 承認者のページは走らせた場所によらない() {
    let rule = root().join("tests/corpus/出荷の送料.rule");
    let rule = rule.to_str().unwrap();
    let dir = std::env::temp_dir().join(format!("rulec-connect-page-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(&dir)
        .args(["gen", rule, "--out", "out"])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let page = std::fs::read_to_string(dir.join("out/python/shipment_fee_page.html")).unwrap();
    assert!(page.contains("_proto(shipment, [[\"declaredValueJpy\", \"declared_value_jpy\"]]"), "protojson の名前で読んでいない");
    for cwd in [root(), dir.clone(), std::env::temp_dir()] {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .current_dir(&cwd)
            .args(["doc", rule, "--format", "html"])
            .output()
            .expect("rulec を起動できない");
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim_end(), page.trim_end(), "{} から描いたページが違う", cwd.display());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// What `rulec api` says about the wire is what the `.proto` says, read back by rulec's own
/// reader: each value's name and number, value 0 as "not set", and every field a caller
/// sends marked `optional` so that one left out is seen (§15.160).
#[test]
fn connect_の目録は生成した_proto_と一致する() {
    let rules = [RULE, "tests/corpus/全国運賃.rule"];
    let dir = generate("inventory", &rules);
    for (rule, alias, version) in [(RULE, "shipping_fee", "v4"), ("tests/corpus/全国運賃.rule", "freight", "v1")] {
        let a = api(rule);
        let c = a.get("connect").expect("connect の項が無い");
        let body = std::fs::read_to_string(dir.join(format!("proto/rulec/{alias}/{version}/{alias}.proto"))).unwrap();
        let declared = rulec::proto::enums(&body);
        let enums = c.get("enums").map(items).expect("enums が無い");
        assert!(!enums.is_empty(), "{rule}: 列挙が一つも無い");
        for e in enums {
            let ty = e.get("alias").and_then(|x| x.as_str()).unwrap();
            let d = declared.iter().find(|x| x.name == ty).unwrap_or_else(|| panic!("{rule}: {ty} が .proto に無い"));
            assert_eq!(e.get("contract"), Some(&rulec::json::Json::Null));
            let unset = d.unset().map(|v| v.name.clone());
            assert_eq!(e.get("unset").and_then(|x| x.as_str()).map(str::to_string), unset, "{rule}: {ty}");
            let said: Vec<(String, i128)> = e
                .get("values")
                .map(items)
                .unwrap()
                .iter()
                .map(|v| (v.get("alias").and_then(|x| x.as_str()).unwrap().to_string(), v.get("number").and_then(|x| x.as_int()).unwrap()))
                .collect();
            let written: Vec<(String, i128)> = d.named().iter().map(|(_, v)| (v.name.clone(), v.number as i128)).collect();
            assert_eq!(said, written, "{rule}: {ty}");
        }
        // Every field of the request names its enum by the rule's name, which `enums` is keyed by.
        for f in c.get("request_fields").map(items).unwrap() {
            if let Some(en) = f.get("enum").and_then(|x| x.as_str()) {
                assert!(enums.iter().any(|e| e.get("name").and_then(|x| x.as_str()) == Some(en)), "{rule}: {en}");
            }
        }
        let request = body.split("message DecideRequest {").nth(1).unwrap().split('}').next().unwrap();
        for line in request.lines().map(str::trim).filter(|l| l.ends_with(';')) {
            assert!(line.starts_with("optional ") || line.starts_with("repeated "), "{rule}: {line}");
        }
    }
    // A rule that walks a sequence describes the element's fields as well.
    let c = api("tests/corpus/全国運賃.rule");
    let fields = c.get("connect").and_then(|c| c.get("element_fields")).map(items).expect("element_fields");
    assert!(fields.iter().any(|f| f.get("enum").and_then(|x| x.as_str()) == Some("ゾーン区分")), "要素の列挙の項が無い");
    assert_eq!(api(RULE).get("connect").and_then(|c| c.get("element_fields")), Some(&rulec::json::Json::Null));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Names the way buf splits them. buf lint asks for `HTTP_METHOD_` in front of the values of
/// `HTTPMethod`, so a contract that passes it is read without E032, and a rule's own enum of
/// that name is written with that prefix. A value whose name would be left starting with a
/// digit keeps its prefix, which is an alias a rule can declare (§15.160).
#[test]
fn 名前は_buf_と同じに切る() {
    let d = std::env::temp_dir().join(format!("rulec-connect-split-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("web.proto"),
        "syntax = \"proto3\";\npackage web.v1;\nenum HTTPMethod {\n  HTTP_METHOD_UNSPECIFIED = 0;\n  HTTP_METHOD_GET = 1;\n  HTTP_METHOD_POST = 2;\n}\n\
         enum Size {\n  SIZE_UNSPECIFIED = 0;\n  SIZE_60 = 1;\n  SIZE_80 = 2;\n}\n",
    )
    .unwrap();
    std::fs::write(
        d.join("fee.rule"),
        "rule 手数料(fee) v1\n\n\
         import proto \"web.proto\" HTTPMethod -> 方式\n\
         import proto \"web.proto\" Size -> 大きさ\n\
         enum 方式(verb) = 取得(get) | 送信(post)\n\
         enum 大きさ(size) = 六十(size_60) | 八十(size_80)\n\
         enum 経路(HTTPRoute) = 内(inner) | 外(outer)\n\n\
         inputs\n  方式(m) : 方式\n  大きさ(s) : 大きさ\n  経路(r) : 経路\n\n\
         outputs\n  手数料(amount) : money[円]  round down(1円)\n\n\
         table 表(t)\npolicy first\n| 方式 | 大きさ | 経路 | -> 手数料(amount) : money[円] |\n\
         | 取得 | 六十 | 内 | 10円 |\n| 送信 | 八十 | 外 | 20円 |\n| - | - | - | 30円 |\n",
    )
    .unwrap();
    let rule = d.join("fee.rule");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .args(["check", rule.to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert_eq!(o.status.code(), Some(0), "{said}");
    assert!(!said.contains("E032"), "{said}");
    let out = d.join("out");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .args(["gen", rule.to_str().unwrap(), "--out", out.to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let body = std::fs::read_to_string(out.join("proto/rulec/fee/v1/fee.proto")).unwrap();
    assert!(body.contains("enum HTTPRoute {\n  HTTP_ROUTE_UNSPECIFIED = 0;\n  HTTP_ROUTE_INNER = 1;"), "{body}");
    let svc = std::fs::read_to_string(out.join("python/fee_service.py")).unwrap();
    assert!(svc.contains("web_v1_web_pb.Size(1): m.Size.SIZE60,  # SIZE_60"), "{svc}");
    if have("buf") {
        let o = Command::new("buf").current_dir(out.join("proto")).arg("lint").output().expect("buf を起動できない");
        assert!(o.status.success(), "buf lint:\n{}", String::from_utf8_lossy(&o.stdout));
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// Two rules may import one contract, but not two different files under one path: the second
/// would overwrite the first, and one of the two services would speak the other's enum.
#[test]
fn 同じパスに違う契約は写さない() {
    let d = std::env::temp_dir().join(format!("rulec-connect-clash-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for (sub, extra) in [("a", ""), ("b", "  MEMBER_TIER_PLATINUM = 3;\n")] {
        let at = d.join(sub);
        std::fs::create_dir_all(&at).unwrap();
        std::fs::write(
            at.join("order.proto"),
            format!("syntax = \"proto3\";\npackage shop.v1;\nenum MemberTier {{\n  MEMBER_TIER_UNSPECIFIED = 0;\n  MEMBER_TIER_BASIC = 1;\n  MEMBER_TIER_GOLD = 2;\n{extra}}}\n"),
        )
        .unwrap();
        let platinum = if extra.is_empty() { "" } else { " | 白金(platinum) default" };
        std::fs::write(
            at.join(format!("{sub}.rule")),
            format!(
                "rule 料金{sub}(fee_{sub}) v1\n\nimport proto \"order.proto\" MemberTier -> 会員区分\n\
                 enum 会員区分(tier) = 一般(basic) | ゴールド(gold) default{platinum}\n\n\
                 inputs\n  会員(m) : 会員区分\n\noutputs\n  送料(fee) : money[円]  round up(10円)\n\n\
                 table 表(t)\npolicy first\n| 会員 | -> 送料(fee) : money[円] |\n| 一般 | 800円 |\n| - | 400円 |\n"
            ),
        )
        .unwrap();
    }
    let a = d.join("a/a.rule");
    let b = d.join("b/b.rule");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .args(["gen", a.to_str().unwrap(), b.to_str().unwrap(), "--out", d.join("out").to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    let err = String::from_utf8_lossy(&o.stderr).into_owned();
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("proto/shop/v1/order.proto"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The service as `gen` leaves it, stood up with nothing added to the path: the contracts it
/// imports an enum from are in the module, the stubs are built from it, and the service
/// answers. What it refuses is what a hand-written request gets wrong — a field or an enum
/// value it does not know, and an input left out, which proto3 would otherwise read as zero.
/// A contract whose value 0 is a value of its own is where that mattered: left out, the
/// field was decided as that value (§15.160). `rulec test` drives the same service over the
/// vectors.
#[test]
fn 生成したまま立ち_名前の誤りと省いた入力を断る() {
    let importable = |m: &str| {
        Command::new("python3").args(["-c", &format!("import {m}")]).output().map(|o| o.status.success()).unwrap_or(false)
    };
    if !(have("buf") && have("protoc-gen-py") && have("protoc-gen-connectrpc") && importable("connectrpc")) {
        eprintln!("buf・プラグイン・connectrpc のどれかが無いので飛ばします");
        return;
    }
    let d = std::env::temp_dir().join(format!("rulec-connect-stand-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("order.proto"),
        "syntax = \"proto3\";\npackage shop.v1;\nenum MemberTier {\n  MEMBER_TIER_UNSPECIFIED = 0;\n  MEMBER_TIER_BASIC = 1;\n  MEMBER_TIER_GOLD = 2;\n}\n",
    )
    .unwrap();
    std::fs::write(
        d.join("status.proto"),
        "syntax = \"proto3\";\npackage acct.v1;\nenum Status {\n  STATUS_ACTIVE = 0;\n  STATUS_CLOSED = 1;\n}\n",
    )
    .unwrap();
    std::fs::write(
        d.join("delivery.rule"),
        "rule 配送料金(delivery_fee) v1\n\n\
         import proto \"order.proto\" MemberTier -> 会員区分\n\
         import proto \"status.proto\" Status -> 状態\n\
         enum 会員区分(tier) = 一般(basic) | ゴールド(gold) default\n\
         enum 状態(status) = 有効(active) default | 停止(closed)\n\n\
         inputs\n  会員(m) : 会員区分\n  状態(s) : 状態\n  急ぎ(express) : bool\n  金額(amount) : money[円]  range >=0円 <=100000円\n\n\
         outputs\n  送料(fee) : money[円]  round up(10円)\n\n\
         table 送料表(fee_table)\npolicy first\n\
         | 状態 | 会員 | 急ぎ | 金額 | -> 送料(fee) : money[円] |\n\
         | 停止 | - | - | - | 0円 |\n\
         | - | 一般 | true | - | 1200円 |\n\
         | - | 一般 | false | <5000円 | 800円 |\n\
         | - | - | - | - | 400円 |\n",
    )
    .unwrap();
    let out = d.join("out");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .args(["gen", d.join("delivery.rule").to_str().unwrap(), "--out", out.to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    // Only the Python side is kept: what is under test here is the service, and `rulec test`
    // runs what it finds.
    for e in std::fs::read_dir(&out).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if !["python", "proto", "vectors"].contains(&n.as_str()) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .args(["test", out.to_str().unwrap()])
        .output()
        .expect("rulec を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success() && said.contains("delivery_fee (Python, Connect/WSGI) "), "{said}{}", String::from_utf8_lossy(&o.stderr));
    assert!(!said.contains("FAIL"), "{said}");
    // The stubs, the way the documentation builds them, but with the plugins on this machine.
    let o = Command::new("buf")
        .current_dir(out.join("proto"))
        .args([
            "generate",
            "--template",
            r#"{"version":"v2","plugins":[{"local":"protoc-gen-py","out":"../python/stubs","strategy":"all"},{"local":"protoc-gen-connectrpc","out":"../python/stubs","strategy":"all"}]}"#,
        ])
        .output()
        .expect("buf を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let client = r#"
import json, subprocess, sys, urllib.error, urllib.request

p = subprocess.Popen([sys.executable, "-B", "delivery_fee_service.py", "--http", "127.0.0.1:0"],
                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
base = p.stdout.readline().strip()
if not base.startswith("http"):
    p.wait(timeout=10)
    sys.exit("did not start: " + p.stderr.read())
out = []
try:
    for body in json.loads(sys.argv[1]):
        req = urllib.request.Request(base + "/rulec.delivery_fee.v1.DeliveryFeeService/Decide",
                                     data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req) as r:
                out.append([r.status, json.loads(r.read())])
        except urllib.error.HTTPError as e:
            out.append([e.code, json.loads(e.read())])
finally:
    p.kill()
print(json.dumps(out, ensure_ascii=False))
"#;
    let ok = r#"{"m": "MEMBER_TIER_GOLD", "s": "STATUS_ACTIVE", "express": false, "amount": "1000"}"#;
    let bodies = format!(
        "[{ok}, \
          {{\"m\": \"MEMBER_TIER_BASIC\", \"s\": \"STATUS_ACTIVE\", \"express\": true, \"amount\": \"0\"}}, \
          {{\"m\": \"MEMBER_TIER_GOLD\", \"express\": false, \"amount\": \"1000\"}}, \
          {{\"m\": \"MEMBER_TIER_GOLD\", \"s\": \"STATUS_ACTIVE\", \"amount\": \"1000\"}}, \
          {{\"m\": \"MEMBER_TIER_GOLD\", \"s\": \"STATUS_CLOSD\", \"express\": false, \"amount\": \"1000\"}}, \
          {{\"m\": \"MEMBER_TIER_GOLD\", \"s\": \"STATUS_ACTIVE\", \"express\": false, \"amonut\": \"1000\"}}, \
          {{\"m\": \"MEMBER_TIER_UNSPECIFIED\", \"s\": \"STATUS_ACTIVE\", \"express\": false, \"amount\": \"1000\"}}]"
    );
    let o = Command::new("python3")
        .current_dir(out.join("python"))
        .env_remove("PYTHONPATH")
        .args(["-c", client, &bodies])
        .output()
        .expect("python3 を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let got = rulec::json::parse(String::from_utf8_lossy(&o.stdout).trim()).expect("JSON でない");
    let got: Vec<(i128, String)> = items(&got)
        .iter()
        .map(|r| {
            let r = items(r);
            let body = &r[1];
            let what = body
                .get("fee")
                .and_then(|x| x.as_str())
                .map(|f| format!("fee {f}"))
                .or_else(|| body.get("message").and_then(|x| x.as_str()).map(str::to_string))
                .unwrap_or_default();
            (r[0].as_int().unwrap(), what)
        })
        .collect();
    assert_eq!(got[0], (200, "fee 400".to_string()));
    assert_eq!(got[1], (200, "fee 1200".to_string()), "0 を明示した入力は受け取る");
    assert_eq!(got[2], (400, "状態: not set".to_string()), "省いた列挙が 0 番の値として判断された");
    assert_eq!(got[3], (400, "急ぎ: not set".to_string()));
    assert_eq!(got[4].0, 400, "知らない列挙の名前が通った: {}", got[4].1);
    assert!(got[4].1.contains("STATUS_CLOSD"), "{}", got[4].1);
    assert_eq!(got[5].0, 400, "知らないフィールドが通った: {}", got[5].1);
    assert!(got[5].1.contains("amonut"), "{}", got[5].1);
    assert_eq!(got[6].0, 400, "未設定の値が通った: {}", got[6].1);
    let _ = std::fs::remove_dir_all(&d);
}

/// A pin of protovalidate as `buf dep update` writes it, for the fixtures below. The tests
/// that read it fetch nothing; the one that builds the module does, through buf.
const PROTOVALIDATE_PIN: &str = "  - name: buf.build/bufbuild/protovalidate\n    commit: 511051f7f4374c3ca873b53ae68a9288\n    digest: b5:a4a2d4d808a25984cced60769c822c5d496ef0b740f56ac0c9e6b97aaa25b86a9332a00ffd74e0cd202be29e91bd3edfb0bf2ba4dacfe48ff2d8217f9986e3c8\n";

/// A contract that imports protovalidate, in a buf workspace of its own beside a rule that
/// imports its enum: `<d>/proto/shop/v1/order.proto` under `<d>/proto/buf.yaml`, and the rule
/// at `<d>/rules/fee.rule`. `lock` is the workspace's `buf.lock`, when it has one.
fn bsr_fixture(tag: &str, buf_yaml: Option<&str>, lock: Option<&str>) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-connect-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("proto/shop/v1")).unwrap();
    std::fs::create_dir_all(d.join("rules")).unwrap();
    std::fs::write(
        d.join("proto/shop/v1/order.proto"),
        "syntax = \"proto3\";\npackage shop.v1;\nimport \"buf/validate/validate.proto\";\n\
         enum MemberTier {\n  MEMBER_TIER_UNSPECIFIED = 0;\n  MEMBER_TIER_BASIC = 1;\n  MEMBER_TIER_GOLD = 2;\n}\n\
         message Order {\n  int64 weight_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 40000}];\n}\n",
    )
    .unwrap();
    if let Some(y) = buf_yaml {
        std::fs::write(d.join("proto/buf.yaml"), y).unwrap();
    }
    if let Some(l) = lock {
        std::fs::write(d.join("proto/buf.lock"), l).unwrap();
    }
    std::fs::write(
        d.join("rules/fee.rule"),
        "rule 配送料金(delivery_fee) v1\n\n\
         import proto \"../proto/shop/v1/order.proto\" MemberTier -> 会員区分\n\
         enum 会員区分(tier) = 一般(basic) | ゴールド(gold) default\n\n\
         inputs\n  会員(m) : 会員区分\n\n\
         outputs\n  送料(fee) : money[円, incl_tax]  round up(10円)\n\n\
         table 送料表(fee_table)\npolicy first\n\
         | 会員 | -> 送料(fee) : money[円, incl_tax] |\n| 一般 | 800円 |\n| - | 400円 |\n",
    )
    .unwrap();
    d
}

/// `gen` run in `d` over its rules: the exit code and what it said on stderr.
fn gen_in(d: &std::path::Path, rules: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(d)
        .env("RULEC_LANG", "en")
        .arg("gen")
        .args(rules)
        .args(["--out", "out"])
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// What a contract imports from a BSR module is not on disk, so the module declares the
/// module and pins it, taking both from the contract's own `buf.yaml` and `buf.lock`
/// (§15.161). Of the modules that `buf.yaml` declares, the one the import comes from is
/// declared; every pin of the lock is carried, since a module's own dependencies are pinned
/// there too. The stubs of those files are written with the rest (`include_imports`).
#[test]
fn 契約の依存を_buf_yaml_と_buf_lock_に引き継ぐ() {
    let lock = format!(
        "# Generated by buf. DO NOT EDIT.\nversion: v2\ndeps:\n{PROTOVALIDATE_PIN}  - name: buf.build/googleapis/googleapis\n    commit: c17df5b2beca46928cc87d5656bd5343\n    digest: b5:648a\n"
    );
    let d = bsr_fixture(
        "deps",
        Some("version: v2\ndeps:\n  - buf.build/bufbuild/protovalidate\n  - buf.build/googleapis/googleapis\n"),
        Some(&lock),
    );
    let (code, err) = gen_in(&d, &["rules/fee.rule"]);
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("warning"), "{err}");
    let yaml = std::fs::read_to_string(d.join("out/proto/buf.yaml")).unwrap();
    assert!(yaml.contains("deps:\n  - buf.build/bufbuild/protovalidate\nlint:"), "{yaml}");
    let got = std::fs::read_to_string(d.join("out/proto/buf.lock")).expect("buf.lock が書かれていない");
    assert!(got.contains(PROTOVALIDATE_PIN) && got.contains("buf.build/googleapis/googleapis"), "{got}");
    let gen_yaml = std::fs::read_to_string(d.join("out/proto/buf.gen.yaml")).unwrap();
    assert!(gen_yaml.contains("include_imports: true"), "{gen_yaml}");
    let a = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(&d).args(["api", "rules/fee.rule"]).output().unwrap();
    let a = rulec::json::parse(&String::from_utf8_lossy(&a.stdout)).expect("JSON でない");
    let deps: Vec<&str> = a.get("connect").and_then(|c| c.get("deps")).map(items).unwrap().iter().filter_map(|x| x.as_str()).collect();
    assert_eq!(deps, ["buf.build/bufbuild/protovalidate"]);
    // A rule with no contract leaves the module as it was: no dependency, no lock.
    let plain = generate("nodeps", &[RULE]);
    assert!(!std::fs::read_to_string(plain.join("proto/buf.yaml")).unwrap().contains("deps:"));
    assert!(!plain.join("proto/buf.lock").exists());
    let _ = std::fs::remove_dir_all(&plain);
    let _ = std::fs::remove_dir_all(&d);
}

/// When nothing beside the contract pins the module, `gen` declares what it can and says what
/// to run; it fetches nothing itself. The same when the lock is in v1's shape, whose digests a
/// v2 module does not read. Two locks that pin one module at two commits are an error: one
/// module holds one.
#[test]
fn 固定できない依存は言い_食い違うピンは止める() {
    let d = bsr_fixture("nopin", None, None);
    let (code, err) = gen_in(&d, &["rules/fee.rule"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("buf.build/bufbuild/protovalidate") && err.contains("buf dep update"), "{err}");
    assert!(std::fs::read_to_string(d.join("out/proto/buf.yaml")).unwrap().contains("  - buf.build/bufbuild/protovalidate\n"));
    assert!(!d.join("out/proto/buf.lock").exists(), "固定できないのに buf.lock を書いた");
    let _ = std::fs::remove_dir_all(&d);

    let v1 = "version: v1\ndeps:\n  - remote: buf.build\n    owner: bufbuild\n    repository: protovalidate\n    commit: 511051f7f4374c3ca873b53ae68a9288\n    digest: shake256:b911\n";
    let d = bsr_fixture("v1lock", Some("version: v1\ndeps:\n  - buf.build/bufbuild/protovalidate\n"), Some(v1));
    let (code, err) = gen_in(&d, &["rules/fee.rule"]);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("v1") && err.contains("buf config migrate"), "{err}");
    assert!(!d.join("out/proto/buf.lock").exists());
    let _ = std::fs::remove_dir_all(&d);

    // Two workspaces, one module, two commits.
    let a = bsr_fixture("pin-a", Some("version: v2\ndeps:\n  - buf.build/bufbuild/protovalidate\n"), Some(&format!("version: v2\ndeps:\n{PROTOVALIDATE_PIN}")));
    let b = bsr_fixture("pin-b", Some("version: v2\ndeps:\n  - buf.build/bufbuild/protovalidate\n"), Some(&format!("version: v2\ndeps:\n{}", PROTOVALIDATE_PIN.replace("511051f7", "00000000"))));
    // The second contract is another package, so that only the pins meet.
    let other = std::fs::read_to_string(b.join("proto/shop/v1/order.proto")).unwrap().replace("package shop.v1;", "package shop.v2;");
    std::fs::create_dir_all(b.join("proto/shop/v2")).unwrap();
    std::fs::write(b.join("proto/shop/v2/order.proto"), other).unwrap();
    let rule = std::fs::read_to_string(b.join("rules/fee.rule")).unwrap().replace("shop/v1/", "shop/v2/").replace("(delivery_fee)", "(delivery_fee_b)");
    std::fs::write(b.join("rules/fee.rule"), rule).unwrap();
    let (code, err) = gen_in(&a, &["rules/fee.rule", b.join("rules/fee.rule").to_str().unwrap()]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("511051f7") && err.contains("00000000"), "{err}");
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

/// The module builds with its dependency, the stubs of protovalidate's file are written with
/// the rest, and the service answers — as `gen` left it, and through `rulec test`. buf
/// fetches the module, so this is the one test here that reaches the network; with none, it
/// says so and stops.
#[test]
fn 依存のある契約のサービスが立つ() {
    let importable = |m: &str| {
        Command::new("python3").args(["-c", &format!("import {m}")]).output().map(|o| o.status.success()).unwrap_or(false)
    };
    if !(have("buf") && have("protoc-gen-py") && have("protoc-gen-connectrpc") && importable("connectrpc")) {
        eprintln!("buf・プラグイン・connectrpc のどれかが無いので飛ばします");
        return;
    }
    let d = bsr_fixture(
        "bsr-stand",
        Some("version: v2\ndeps:\n  - buf.build/bufbuild/protovalidate\n"),
        Some(&format!("version: v2\ndeps:\n{PROTOVALIDATE_PIN}")),
    );
    let (code, err) = gen_in(&d, &["rules/fee.rule"]);
    assert_eq!(code, 0, "{err}");
    let out = d.join("out");
    let o = Command::new("buf").current_dir(out.join("proto")).args(["build", "-o", "/dev/null"]).output().expect("buf を起動できない");
    let said = String::from_utf8_lossy(&o.stderr).into_owned();
    if !o.status.success()
        && ["no such host", "dial tcp", "connection refused", "i/o timeout", "unavailable", "TLS handshake", "network is unreachable"]
            .iter()
            .any(|w| said.contains(w))
    {
        eprintln!("BSR に届かないので飛ばします: {said}");
        let _ = std::fs::remove_dir_all(&d);
        return;
    }
    assert!(o.status.success(), "buf build が断りました:\n{said}");
    for e in std::fs::read_dir(&out).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if !["python", "proto", "vectors"].contains(&n.as_str()) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").args(["test", out.to_str().unwrap()]).output().unwrap();
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success() && said.contains("delivery_fee (Python, Connect/WSGI) "), "{said}{}", String::from_utf8_lossy(&o.stderr));
    assert!(!said.contains("FAIL"), "{said}");
    assert!(out.join("python/stubs/buf/validate/validate_pb.py").is_file(), "protovalidate の stub が書かれていない");
    let _ = std::fs::remove_dir_all(&d);
}
