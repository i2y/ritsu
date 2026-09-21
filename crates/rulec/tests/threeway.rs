//! Agreement across every implementation (§8.5, §9.3).
//!
//! Feed the same vectors to all of them — the reference evaluator and the generated Python,
//! TypeScript, Rust, Ruby and Go — and check that the canonical JSON matches byte
//! for byte. It needs neither data nor a legacy implementation, which makes it the main
//! guarantee of M1.
//!
//! Where a toolchain is missing, only that language is skipped, and the skip is always
//! reported — a silent skip would be a green run that proved nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    // go takes `go version`, python3 takes `python3 --version`. Try both.
    // Trying only one and skipping silently would go green without anything having run.
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

fn rulec(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    assert!(out.status.success(), "rulec {args:?} が失敗: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Every rule in the corpus, with the name its generated files take. Hand-written lists rot:
/// two rules had fallen out of this one, and one of them was returning a number a hundred
/// times too large in all four languages while the suite stayed green. `コーパスは全部載っている`
/// keeps the list honest.
const CORPUS: &[(&str, &str)] = &[
    ("tests/corpus/品番の扱い.rule", "sku_handling"),
    ("tests/corpus/買物かごの送料.rule", "cart_shipping"),
    ("tests/corpus/比例配分.rule", "pro_rata"),
    ("tests/corpus/ゆうパック運賃.rule", "yupack_fee"),
    ("tests/corpus/クーポン割引.rule", "coupon_discount"),
    ("tests/corpus/クーポン併用.rule", "coupon_stack"),
    ("tests/corpus/送料.rule", "shipping_fee"),
    ("tests/corpus/期間区分.rule", "period"),
    ("tests/corpus/適用順序.rule", "apply_order"),
    ("tests/corpus/クーポン一枚.rule", "coupon_step"),
    ("tests/corpus/決済手数料.rule", "payment_fee"),
    ("tests/corpus/ポイント付与.rule", "points"),
    ("tests/corpus/評価ランク.rule", "rank"),
    ("tests/corpus/値引の充当.rule", "discount_fill"),
    ("tests/corpus/会員特典.rule", "member_perk"),
    ("tests/corpus/ec261.rule", "ec261"),
    ("tests/corpus/健康保険料.rule", "kenpo_premium"),
    ("tests/corpus/厚生年金保険料.rule", "pension_premium"),
    ("tests/corpus/所得税.rule", "income_tax"),
    ("tests/corpus/領収書の印紙税.rule", "receipt_stamp"),
    ("tests/corpus/印紙税.rule", "stamp_duty"),
    ("tests/corpus/印紙税の本則と軽減.rule", "stamp_duty_split"),
    ("tests/corpus/送料のただし書.rule", "shipping_proviso"),
    ("tests/corpus/退職手当.rule", "retirement_allowance"),
    ("tests/corpus/非常勤退職手当.rule", "part_time_allowance"),
    ("tests/corpus/全国運賃.rule", "freight"),
    ("tests/corpus/納入先照合.rule", "supplier_match"),
    ("tests/corpus/保存基準.rule", "storage"),
    ("tests/corpus/事務所の衛生基準.rule", "office_standard"),
    ("tests/corpus/Claude利用料.rule", "claude_usage_fee"),
    ("tests/corpus/預け荷物料金.rule", "baggage_fee"),
    ("tests/corpus/予約取消可否.rule", "cancel_verdict"),
    ("tests/corpus/補償証明書.rule", "compensation"),
    ("tests/corpus/parcel_rate.rule", "parcel_rate"),
    ("tests/corpus/return_eligibility.rule", "return_eligibility"),
    ("tests/corpus/uk_minimum_wage.rule", "uk_minimum_wage"),
    ("tests/corpus/uk_income_tax.rule", "uk_income_tax"),
    ("tests/corpus/us_income_tax.rule", "us_income_tax"),
    ("tests/corpus/osha_extinguisher.rule", "osha_extinguisher"),
];

#[test]
fn コーパスは全部載っている() {
    let dir = root().join("tests/corpus");
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .map(|p| format!("tests/corpus/{}", p.file_name().unwrap().to_string_lossy()))
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = CORPUS.iter().map(|(f, _)| (*f).to_string()).collect();
    listed.sort();
    assert_eq!(
        on_disk, listed,
        "コーパスに足した規則が一致検査の一覧に載っていません。tests/threeway.rs の CORPUS に足してください"
    );
}

#[test]
fn 評価器と生成コードが全言語で一致する() {
    let dir = std::env::temp_dir().join(format!("rulec-3way-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();

    let files: Vec<&str> = CORPUS.iter().map(|(f, _)| *f).collect();
    let mut args = vec!["gen"];
    args.extend(files.iter().copied());
    args.push("--out");
    args.push(&out);
    rulec(&args);

    // Which languages can run here. The set is src/backend.rs; this file used to name
    // each one twice and a sixth would have needed both spots (§15.20).
    // `tool` is not the whole story. A backend can need something else on top of the command
    // it names — numpy under `python3`, node and a wasm target under `rustc` — which is what
    // `ready` says. Filtering on the tool alone made this suite fail on a machine that was
    // only missing the extra, where `rulec test` would have skipped the language and said so.
    let ready = |b: &rulec::backend::Backend| b.ready.map(|r| r()).unwrap_or(Ok(()));
    let present: Vec<&rulec::backend::Backend> =
        rulec::backend::ALL.iter().filter(|b| have(b.tool) && ready(b).is_ok()).collect();
    for b in rulec::backend::ALL {
        if !present.iter().any(|p| p.id == b.id) {
            match ready(b) {
                Err(why) if have(b.tool) => eprintln!("注意: {why}"),
                _ => eprintln!("注意: {} が無いので {} を飛ばした", b.tool, b.name),
            }
        }
    }
    assert!(!present.is_empty(), "どの toolchain も無いので一致を確かめられない");

    let mut total = 0usize;
    for (file, alias) in CORPUS {
        let vec_path = dir.join("vectors").join(format!("{alias}.jsonl"));
        let exp = std::fs::read_to_string(dir.join("vectors").join(format!("{alias}.expected.jsonl")))
            .expect("期待値が無い");
        let vectors = std::fs::read_to_string(&vec_path).expect("ベクタが無い");
        assert!(!vectors.trim().is_empty(), "{alias}: ベクタがゼロ件");
        total += vectors.lines().count();
        let pkg = alias.replace('_', "");

        // A rule that walks a sequence — folding it or counting it — is not generated for a
        // backend that has no walk (SQL: one query has no place to carry a value from row to
        // row, §15.56, §15.58).
        let src = std::fs::read_to_string(root().join(file)).unwrap_or_default();
        let walks = src.lines().any(|l| l.starts_with("elements "));
        // The same for a table whose column is a `string` (§15.101): the backends that
        // decline one write no files for it.
        let texts = src.lines().any(|l| l.contains("starts_with "));

        for b in present.iter().filter(|b| (b.folds || !walks) && (b.texts || !texts)) {
            let plan = (b.run)(alias, &pkg);
            let cwd = dir.join(&plan.cwd);
            if let Some((cmd, args)) = &plan.build {
                let built = Command::new(cmd)
                    .current_dir(&cwd)
                    .args(args)
                    .output()
                    .unwrap_or_else(|e| panic!("{}: {cmd} を起動できない: {e}", b.name));
                assert!(
                    built.status.success(),
                    "{alias}: 生成した {} がコンパイルできない:\n{}",
                    b.name,
                    String::from_utf8_lossy(&built.stderr)
                );
            }
            let o = Command::new(&plan.cmd)
                .current_dir(&cwd)
                .args(&plan.args)
                .stdin(std::fs::File::open(&vec_path).unwrap())
                .output()
                .unwrap_or_else(|e| panic!("{}: 起動できない: {e}", b.name));
            let got = String::from_utf8_lossy(&o.stdout).into_owned();
            assert!(
                o.status.success(),
                "{alias}: {} が落ちた: {}",
                b.name,
                String::from_utf8_lossy(&o.stderr)
            );
            assert_eq!(got, exp, "{alias}: 評価器と生成 {} が食い違う", b.name);
        }
    }
    eprintln!(
        "一致: 規則 {} 本 / ベクタ {total} 件（{}）",
        CORPUS.len(),
        present.iter().map(|b| b.name).collect::<Vec<_>>().join(" / ")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 生成物は決定的である() {
    // §8.5: the same .rule and the same rulec give byte-identical output. Any leakage of hash order
    // is a failure.
    let mk = |tag: &str| -> Vec<(String, String)> {
        let dir = std::env::temp_dir().join(format!("rulec-det-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = dir.to_string_lossy().to_string();
        let mut args = vec!["gen"];
        let files: Vec<&str> = CORPUS.iter().map(|(f, _)| *f).collect();
        args.extend(files.iter().copied());
        args.push("--out");
        args.push(&out);
        rulec(&args);
        let mut v: Vec<(String, String)> = Vec::new();
        collect(&dir, &dir, &mut v);
        v.sort();
        let _ = std::fs::remove_dir_all(&dir);
        v
    };
    assert_eq!(mk("a"), mk("b"), "二度生成して食い違った");
}

fn collect(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(base, &p, out);
        } else if let Ok(s) = std::fs::read_to_string(&p) {
            out.push((p.strip_prefix(base).unwrap().to_string_lossy().into(), s));
        }
    }
}

#[test]
fn 生成物は両言語の整形器に素で通る() {
    // §8.1 criterion 5. The output is formatted at generation time rather than by running gofmt
    // afterwards (running it afterwards makes the output environment-dependent and breaks the
    // determinism of §8.5).
    if !have("go") {
        eprintln!("注意: go が無いので gofmt の検査を飛ばした");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-fmt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let mut args = vec!["gen"];
    let files: Vec<&str> = CORPUS.iter().map(|(f, _)| *f).collect();
    args.extend(files.iter().copied());
    args.push("--out");
    args.push(&out);
    rulec(&args);

    let go_dir = dir.join("go");
    let o = Command::new("gofmt")
        .arg("-l")
        .arg(&go_dir)
        .output()
        .expect("gofmt を起動できない");
    let listed = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(listed.trim().is_empty(), "gofmt が直したいファイルがある:\n{listed}");

    // Python must at least parse.
    for (_, alias) in CORPUS {
        let p = dir.join("python").join(format!("{alias}.py"));
        let o = Command::new("python3")
            .args(["-c", "import ast,sys; ast.parse(open(sys.argv[1],encoding='utf-8').read())"])
            .arg(&p)
            .output()
            .expect("python3 を起動できない");
        assert!(o.status.success(), "{alias}.py が構文エラー: {}", String::from_utf8_lossy(&o.stderr));
    }

    // The PEP 8 side (the counterpart of an empty `gofmt -l`). Unlike Go, Python has no single
    // formatter, so the claim is split in two and measured separately.
    let Some(ruff) = ruff() else {
        eprintln!("注意: ruff が無いので PEP 8 の検査を飛ばした");
        let _ = std::fs::remove_dir_all(&dir);
        return;
    };
    let py_dir = dir.join("python");

    // (1) The pycodestyle family (E/W) reports nothing, line length aside.
    let o = Command::new(&ruff[0])
        .args(&ruff[1..])
        .args(["check", "--select", "E,W", "--ignore", "E501", "--isolated", "--no-cache"])
        .arg(&py_dir)
        .output()
        .expect("ruff を起動できない");
    assert!(
        o.status.success(),
        "PEP 8（行長を除く）に違反がある:\n{}",
        String::from_utf8_lossy(&o.stdout)
    );

    // (2) The only lines the formatter wants to touch are lines longer than 88 columns.
    // §8.1 demands "one table row per line, with the source cells attached", and that does not fit
    // in 88 columns. A single complaint that is not about wrapping is something the generator
    // should fix, so it fails.
    let o = Command::new(&ruff[0])
        .args(&ruff[1..])
        // `--color never`: ruff colours the diff even into a pipe, and every line then
        // begins with an escape rather than with `-`, so the filter below matched nothing
        // and the test passed by looking at nothing. CI caught what this did not (§15.105).
        .args(["format", "--check", "--diff", "--no-cache", "--color", "never"])
        .arg(&py_dir)
        .output()
        .expect("ruff を起動できない");
    let diff = String::from_utf8_lossy(&o.stdout).into_owned();
    let short: Vec<&str> = diff
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .map(|l| &l[1..])
        .filter(|l| rulec::diag::width(l) <= 88)
        .collect();
    assert!(
        short.is_empty(),
        "整形器の指摘のうち、行長で説明できないものがある:\n{}",
        short.join("\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// How to launch ruff. Use it if it is on PATH, otherwise go through uvx. If neither exists, skip.
fn ruff() -> Option<Vec<String>> {
    if have("ruff") {
        return Some(vec!["ruff".into()]);
    }
    let ok = Command::new("uvx")
        .args(["ruff", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then(|| vec!["uvx".into(), "ruff".into()])
}

#[test]
fn 丸めヘルパは両言語で参照実装と一致する() {
    // §8.5: table-level agreement alone lets a helper bug hide in a table that yields no fractions.
    let dir = std::env::temp_dir().join(format!("rulec-round-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    rulec(&["gen", CORPUS[0].0, "--out", &out]);

    if have("python3") {
        let o = Command::new("python3")
            .current_dir(dir.join("python"))
            .arg("_round_test.py")
            .output()
            .expect("python3 を起動できない");
        assert!(o.status.success(), "Python の丸めが合わない: {}", String::from_utf8_lossy(&o.stdout));
    }
    if have("go") {
        let pkg = CORPUS[0].1.replace('_', "");
        let o = Command::new("go")
            .current_dir(dir.join("go").join(&pkg))
            .args(["test", "./..."])
            .output()
            .expect("go を起動できない");
        assert!(o.status.success(), "Go の丸めが合わない: {}", String::from_utf8_lossy(&o.stdout));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `mypy`, the way `ruff` is found: installed, or reachable through `uvx`.
fn mypy() -> Option<Vec<String>> {
    if have("mypy") {
        return Some(vec!["mypy".into()]);
    }
    let ok = Command::new("uvx")
        .args(["mypy", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    ok.then(|| vec!["uvx".into(), "mypy".into()])
}

/// The brands are the reason the Python side declares `NewType` at all, and the README says
/// they "work with mypy and pyright". Nothing checked that, and eight of the thirteen corpus
/// rules did not pass `mypy --strict`: arithmetic on a `NewType` yields the supertype, so
/// the value returned from a rounding helper was a plain `int` where a branded output was
/// declared — exactly the mistake the brand exists to catch (§15.22).
///
/// The runner is checked with the module, because it is also the worked example of how a
/// caller constructs a branded argument.
#[test]
fn 生成pythonはmypy_strictを通る() {
    let Some(my) = mypy() else {
        eprintln!("注意: mypy も uvx も無いので飛ばした");
        return;
    };
    let dir = root().join("target").join("mypy-check");
    let _ = std::fs::remove_dir_all(&dir);
    let mut args: Vec<String> = vec!["gen".into()];
    args.extend(CORPUS.iter().map(|(f, _)| (*f).to_string()));
    args.push("--out".into());
    args.push(dir.to_string_lossy().into_owned());
    rulec(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());

    let o = Command::new(&my[0])
        .args(&my[1..])
        .args(["--strict", "--no-color-output"])
        .arg(dir.join("python"))
        .current_dir(root())
        .output()
        .expect("mypy を起動できない");
    assert!(
        o.status.success(),
        "生成した Python が mypy --strict を通らない:\n{}",
        String::from_utf8_lossy(&o.stdout)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// `steep`, if it is installed. There is no `uvx` for gems, so this is all or nothing.
fn steep() -> bool {
    have("steep") && have("rbs")
}

/// The `.rbs` that ships beside the generated Ruby has to describe the module it sits next
/// to — `steep` checking the module against its own signature is the strongest statement
/// available, because it fails on a method the signature forgot as well as on a wrong type.
///
/// What it cannot do is refuse grams where yen were meant: RBS has no newtype and a type
/// alias is the same type, which is measured rather than assumed in §15.23.
#[test]
fn 生成rubyは自分のrbsでsteepを通る() {
    if !steep() {
        eprintln!("注意: steep か rbs が無いので飛ばした");
        return;
    }
    let dir = root().join("target").join("steep-check");
    let _ = std::fs::remove_dir_all(&dir);
    let mut args: Vec<String> = vec!["gen".into()];
    args.extend(CORPUS.iter().map(|(f, _)| (*f).to_string()));
    args.push("--out".into());
    args.push(dir.to_string_lossy().into_owned());
    rulec(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());

    let rb = dir.join("ruby");
    let modules: Vec<String> = CORPUS.iter().map(|(_, a)| format!("\"{a}.rb\"")).collect();
    std::fs::write(
        rb.join("Steepfile"),
        format!("target :app do\n  signature \"sig\"\n  check {}\nend\n", modules.join(", ")),
    )
    .expect("Steepfile を書けない");

    let o = Command::new("steep").arg("check").current_dir(&rb).output().expect("steep を起動できない");
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(
        out.contains("No type error detected"),
        "生成した Ruby が自分の .rbs で steep を通らない:\n{out}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// And the signature has to bite. A caller that passes a string where the rule declares a
/// number, or a value that is not one of an enum's, has to be refused — otherwise the file
/// is decoration. The enum case is the one `.rbs` buys that plain Ruby cannot: the members
/// are a closed union of the values themselves, not `String`.
#[test]
fn rbsは誤った呼び出しを拒む() {
    if !steep() {
        eprintln!("注意: steep か rbs が無いので飛ばした");
        return;
    }
    let dir = root().join("target").join("steep-bite");
    let _ = std::fs::remove_dir_all(&dir);
    rulec(&["gen", "tests/corpus/クーポン一枚.rule", "--out", dir.to_str().unwrap()]);
    let rb = dir.join("ruby");
    std::fs::write(
        rb.join("caller.rb"),
        "require_relative \"coupon_step\"\n\
         CouponStep.coupon_step(10000, 0, CouponStep::CouponKind::PERCENT, 10, 0, false)\n\
         CouponStep.coupon_step(\"x\", 0, CouponStep::CouponKind::PERCENT, 10, 0, false)\n\
         CouponStep.coupon_step(10000, 0, \"存在しない種別\", 10, 0, false)\n\
         CouponStep.coupon_step(10000, 0, CouponStep::CouponKind::PERCENT, 10, 0)\n",
    )
    .expect("caller を書けない");
    std::fs::write(
        rb.join("Steepfile"),
        "target :app do\n  signature \"sig\"\n  check \"caller.rb\"\nend\n",
    )
    .expect("Steepfile を書けない");

    let o = Command::new("steep").arg("check").current_dir(&rb).output().expect("steep を起動できない");
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    // Three bad calls, one good one. The good one must not be among the complaints.
    assert!(out.contains("caller.rb:3"), "数の型が守られていない:\n{out}");
    assert!(out.contains("caller.rb:4"), "列挙の値が閉じていない:\n{out}");
    assert!(out.contains("caller.rb:5"), "引数の数が守られていない:\n{out}");
    assert!(!out.contains("caller.rb:2"), "正しい呼び出しが拒まれている:\n{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
