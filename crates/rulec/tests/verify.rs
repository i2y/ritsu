//! Equivalence verification (§10). Acceptance for M2.
//!
//! In place of a legacy implementation, stand up an adapter that "just calls the generated Python".
//! Complete agreement is the correct picture; when a defect is deliberately put in, rulec reports
//! counts, clusters, and witnesses.

use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::{Need, TempDir, ready};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The root of ritsu's workspace, two directories above this crate: rulec's site is there, at
/// website/rulec.
fn repo() -> PathBuf {
    root().parent().and_then(Path::parent).expect("the crate is in crates/ of the workspace").to_path_buf()
}

/// Pick the number out of `heading 123` in the output. Counts move with the generator's
/// circumstances, so the tests check the consistency of the breakdown rather than absolute values.
fn n_of(out: &str, head: &str) -> usize {
    out.split(head)
        .nth(1)
        .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

const RULE: &str = "tests/corpus/ゆうパック運賃.rule";
/// The rule the English page shows: a parcel tariff written in English. (It was chosen when the
/// English twin of the rule above still took its prefecture in Japanese; the twin spells it in
/// English now, from `std/jp/prefectures`, DESIGN §15.182.)
const RULE_EN: &str = "tests/corpus/parcel_rate.rule";

/// The same kind of fake legacy implementation, for that rule: it charges a dollar more for every
/// parcel going overseas.
const ADAPTER_EN: &str = r#"# Pretends to be the legacy implementation. It is the generated Python inside, so it matches exactly unless made not to.
import json, sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "python"))
import parcel_rate as legacy

sys.stdin.readline()
print(json.dumps({"ok": True, "impl": "legacy@fake-1"}), flush=True)

BUG = os.environ.get("BUG", "0")
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    d = req["in"]
    got = legacy.parcel_rate(int(d["weight"]), int(d["girth"]), legacy.Zone(d["dest"]), bool(d["signature"]))
    if BUG == "1" and d["dest"] == "overseas":
        got += 1
    print(json.dumps({"id": req["id"], "out": {"fee": got}}, ensure_ascii=False), flush=True)
"#;

const ADAPTER: &str = r#"# 旧実装のふりをする。中身は生成 Python なので、素なら完全に一致する。
import json, sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "python"))
import yupack_fee as legacy

sys.stdin.readline()
print(json.dumps({"ok": True, "impl": "legacy@fake-1"}), flush=True)

BUG = os.environ.get("BUG", "0")
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    d = req["in"]
    got = legacy.yupack_fee(legacy.Prefecture(d["あて先"]), int(d["三辺合計"]), int(d["重量"]))
    if BUG == "1" and d["あて先"] == "沖縄県":
        got += 10
    if BUG == "2" and d["あて先"] == "沖縄県":
        # 旧実装が 1円 の細かさを持っていた場合。ずれは出力の刻み 10円 より必ず小さい。
        got -= 3 + int(d["重量"]) % 5
    if os.environ.get("BROKEN") == "1":
        print("運賃は " + str(got) + " 円", flush=True)
        continue
    # ESCAPE=1 writes the way json.dumps does unless told otherwise: every key and value in \u.
    print(json.dumps({"id": req["id"], "out": {"運賃": got}}, ensure_ascii=os.environ.get("ESCAPE") == "1"), flush=True)
"#;

/// The generated module and the adapter beside it, in a directory of this test's own.
///
/// It used to be named after the process, and the four tests here share one — so they raced:
/// `remove_dir_all` at the start of one ran while another was reading `adapter.py`, and
/// `verify` exited 2 ("cannot read") instead of 1. It stayed green on a laptop for as long as
/// the timing held, and failed the first time CI ran the suite (DESIGN §15.94).
fn setup(tag: &str) -> Option<(TempDir, PathBuf)> {
    setup_for(tag, RULE, ADAPTER)
}

fn setup_for(tag: &str, rule: &str, adapter: &str) -> Option<(TempDir, PathBuf)> {
    if !ready(Need::Python, || have("python3"), "python3 が無いので等価検証を飛ばした") {
        return None;
    }
    let tmp = TempDir::new(&format!("verify-{tag}"));
    let dir = tmp.path().to_path_buf();
    let out = dir.to_string_lossy().to_string();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(root())
        .args(["gen", rule, "--out", &out]).args(rulec::targets::UP_TO_0_40)
        .output()
        .unwrap();
    assert!(o.status.success());
    std::fs::write(dir.join("adapter.py"), adapter).unwrap();
    Some((tmp, dir))
}

fn verify(dir: &PathBuf, bug: &str) -> (i32, String) {
    verify_env(dir, &[("BUG", bug)]).0
}

/// `verify` with the adapter's environment set, and what it wrote to stderr as well.
fn verify_env(dir: &PathBuf, env: &[(&str, &str)]) -> ((i32, String), String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rulec"));
    cmd.current_dir(root())
        .args(["verify", RULE, "--lang", "ja", "--adapter", "python3"])
        .arg(dir.join("adapter.py"));
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.output().expect("rulec を起動できない");
    (
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned()),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn 忠実なアダプタとは完全に一致する() {
    let Some((_tmp, dir)) = setup("忠実なアダプタとは完全に一致する") else { return };
    let (code, out) = verify(&dir, "0");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("(100.000%)"), "{out}");
    assert!(out.contains("不一致はありません"), "{out}");
    // The count itself moves with the generator, so it is not pinned. Only check that it is not
    // empty.
    assert!(n_of(&out, "照合 ") > 50, "ベクタが少なすぎる: {out}");
    assert!(out.contains("legacy@fake-1"), "旧実装の識別子を出す: {out}");
}

#[test]
fn 旧実装の欠陥は件数と証人つきで出る() {
    let Some((_tmp, dir)) = setup("旧実装の欠陥は件数と証人つきで出る") else { return };
    let (code, out) = verify(&dir, "1");
    assert_eq!(code, 1, "不一致があれば 1 で終わる: {out}");
    let total = n_of(&out, "照合 ");
    let agreed = n_of(&out, "一致 ");
    let bad = n_of(&out, "影響 ");
    assert!(bad > 0 && agreed + bad == total, "内訳が合わない: {out}");
    // §10.4: cluster by firing row, and report counts, amounts, and witnesses.
    assert!(out.contains("表 運賃表 行36"), "発火行でクラスタする: {out}");
    // The amount is the cluster total, so it moves with the count. Check the sign, and the
    // difference of the witness's single record.
    assert!(out.contains("差 -"), "金額の差を出す: {out}");
    assert!(out.contains("運賃=1450 / 現行 運賃=1460"), "証人を出す: {out}");
    // A deviation of exactly one grid step is a value mismatch, not rounding. Do not tag it.
    assert!(!out.contains("丸め方の違い"), "刻みちょうどの差を丸めのせいにしている: {out}");
}

/// §10.4: a cluster made up solely of deviations smaller than the output grid gets tagged
/// "suspected rounding difference".
#[test]
fn 刻み未満のずれは丸め方の違いとして括られる() {
    let Some((_tmp, dir)) = setup("刻み未満のずれは丸め方の違いとして括られる") else { return };
    let (code, out) = verify(&dir, "2");
    assert_eq!(code, 1, "{out}");
    assert!(n_of(&out, "影響 ") > 0, "{out}");
    let tagged = out.matches("丸め方の違いの疑い").count();
    let clusters = out.lines().filter(|l| l.starts_with("  表 ")).count();
    assert!(clusters > 0, "クラスタが出ていない: {out}");
    assert_eq!(tagged, clusters, "全クラスタが刻み未満なので全部に付くはず: {out}");
    assert!(out.contains("出力の刻み 10円 未満"), "刻みを書いて根拠を示す: {out}");
}

#[test]
fn 雛形とスキーマが出る() {
    let run = |args: &[&str]| -> String {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .env("RULEC_LANG", "ja")
            .current_dir(root())
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    // The template must work as is (return the handshake, answer line by line).
    let py = run(&["adapter", RULE, "--template", "python"]);
    assert!(py.contains("\"ok\": True"), "握手を返す: {py}");
    assert!(py.contains("あて先"), "入力の名前を案内する: {py}");
    let go = run(&["adapter", RULE, "--template", "go"]);
    assert!(go.contains("package main") && go.contains("bufio"), "{go}");

    // The schema is the wire shape (canonical names, integers in canonical units).
    let sc = run(&["schema", RULE]);
    assert!(sc.contains("\"整数。単位は cm\""), "単位を書く: {sc}");
    assert!(sc.contains("\"minimum\":1"), "範囲を書く: {sc}");
    assert!(sc.contains("\"北海道\""), "列挙を書く: {sc}");
}

/// An answer is read as the JSON it is (§15.151). It used to be searched as text for `"運賃":`,
/// and a key written `"\u904b\u8cc3"` — Python's `json.dumps` unless told otherwise, and PHP's
/// `json_encode` — was never found: an adapter that answered every case right came out at 0%.
#[test]
fn エスケープしたjsonで答えるアダプタとも一致する() {
    let Some((_tmp, dir)) = setup("エスケープしたjsonで答えるアダプタとも一致する") else { return };
    let ((code, out), err) = verify_env(&dir, &[("BUG", "0"), ("ESCAPE", "1")]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("(100.000%)"), "{out}");
}

/// A line that is not JSON stops the run and says which record, rather than being read as an
/// answer with no values in it.
#[test]
fn jsonでない答えは止まって何件目かを言う() {
    let Some((_tmp, dir)) = setup("jsonでない答えは止まって何件目かを言う") else { return };
    let ((code, out), err) = verify_env(&dir, &[("BUG", "0"), ("BROKEN", "1")]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("0 件目の応答") && err.contains("運賃は"), "{err}");
}

/// An optional output with no value is `null` on the wire, which is how the vectors write it;
/// it was compared as the text `null` against `none`, and never matched.
#[test]
fn 任意の出力のnullはnoneとして比べる() {
    if !ready(Need::Python, || have("python3"), "python3 が無いので等価検証を飛ばした") {
        return;
    }
    let tmp = TempDir::new("verify-optional");
    let dir = tmp.path().to_path_buf();
    let rule = "\
rule 任意(opt) v1

enum 区分(kind) = A(a) | B(b)

inputs
  区分(kind) : 区分
  希望(pref) : 区分?

outputs
  割当(slot) : 区分?

table 割当表(slots)
policy unique
| 区分 | -> 割当 |
| A    | 希望    |
| B    | B       |
";
    let adapter = r#"import json, sys
sys.stdin.readline()
print(json.dumps({"ok": True, "impl": "optional"}), flush=True)
for line in sys.stdin:
    r = json.loads(line)
    d = r["in"]
    got = d["希望"] if d["区分"] == "A" else "B"
    print(json.dumps({"id": r["id"], "out": {"割当": got}}), flush=True)
"#;
    std::fs::write(dir.join("opt.rule"), rule).unwrap();
    std::fs::write(dir.join("adapter.py"), adapter).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(&dir)
        .args(["verify", "opt.rule", "--lang", "ja", "--adapter", "python3", "adapter.py"])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert_eq!(o.status.code(), Some(0), "{out}{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.contains("(100.000%)"), "{out}");
}

/// The example on the compare page is what the tool prints: the head of the report in each
/// language, and the JSON beside it. Nothing held it to the output, and the page went on saying
/// 207 cases after the suite had grown to 209 (§15.155). `tests/vdiff.rs` holds the page's
/// `diff` example the same way. The JSON example of docs/formats.md is the English one; it had
/// stayed at an older fake implementation's run until it was taken again (§15.199).
#[test]
fn 文書に載せた実演は_いまの出力と一致する() {
    // Each page shows a rule written in its language: the Japanese page the parcel fee of Japan
    // Post, the English page `parcel_rate`, each with a fake legacy implementation in its names.
    let Some((_tmp, ja_dir)) = setup("文書に載せた実演は_いまの出力と一致する") else { return };
    let Some((_tmp_en, en_dir)) = setup_for("文書に載せた実演は_いまの出力と一致する_en", RULE_EN, ADAPTER_EN) else { return };
    let run = |lang: &str, json: bool| -> String {
        let (rule, dir) = if lang == "en" { (RULE_EN, &en_dir) } else { (RULE, &ja_dir) };
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rulec"));
        cmd.current_dir(root()).args(["verify", rule, "--lang", lang]);
        if json {
            cmd.args(["--format", "json"]);
        }
        cmd.args(["--adapter", "python3"]).arg(dir.join("adapter.py")).env("BUG", "1");
        String::from_utf8_lossy(&cmd.output().expect("rulec を起動できない").stdout).into_owned()
    };
    for (lang, page) in [("en", "website/rulec/docs/compare.md"), ("ja", "website/rulec/docs-ja/compare.md")] {
        let page = std::fs::read_to_string(repo().join(page)).unwrap();
        // The page shows the head of the report: the totals, the counterpart and the lines under
        // it (on the Japanese page, the answers outside what the rule can produce), the first
        // cluster.
        let out = run(lang, false);
        for line in out.lines().take(6).filter(|l| !l.trim().is_empty()) {
            assert!(page.contains(line), "{lang}: 実演に無い行:\n{line}\n出力:\n{out}");
        }
        // And the JSON, in the pieces the page breaks it into.
        let js = run(lang, true);
        let at = |k: &str| js.find(k).unwrap_or_else(|| panic!("{k} が無い:\n{js}"));
        let tail = js.rfind("\"moved\":").expect("moved が無い");
        for piece in [
            &js[..at("\"clusters\":")],
            &js[at("\"clusters\":")..at("\"delta\":")],
            &js[at("\"delta\":")..at("\"witness\":")],
            &js[at("\"witness\":")..at("\"records\":")],
            js[tail..].trim_end(),
        ] {
            assert!(page.contains(piece), "{lang}: JSON の実演に無い部分:\n{piece}\n出力:\n{js}");
        }
    }
    // docs/formats.md shows the English JSON as the example of the shape, broken over more lines:
    // its head, a cluster's difference and its tail are held to the output the same way.
    let js = run("en", true);
    let doc = std::fs::read_to_string(root().join("docs/formats.md")).unwrap();
    let at = |k: &str| js.find(k).unwrap_or_else(|| panic!("{k} が無い:\n{js}"));
    let tail = js.rfind("\"moved\":").expect("moved が無い");
    for piece in [&js[..at("\"clusters\":")], &js[at("\"delta\":")..at("\"witness\":")], js[tail..].trim_end()] {
        assert!(doc.contains(piece), "formats.md: JSON の例に無い部分:\n{piece}\n出力:\n{js}");
    }
}

/// A rule with two outputs and a sequence, for the adapter's side of the protocol.
const PARCEL: &str = "\
rule parcel v1

enum zone = near | far

inputs
  weight : number  range >=1 <=30
  dest   : zone

elements lines
  quantity : number  range >=1 <=10

sum units over lines of quantity  range >=0 <=30

outputs
  fee  : money[JPY]  round down(1JPY)
  days : number  round down(1)

table fees
policy first
| dest | weight | units | -> fee  | -> days |
| near | <=10   | -     | 500JPY  | 1       |
| near | -      | -     | 900JPY  | 2       |
| far  | <=10   | <=5   | 800JPY  | 3       |
| far  | -      | -     | 1500JPY | 4       |
";

/// A stand-in for a legacy implementation that needs no generated code: it answers from the
/// rule's own vectors, so it matches exactly unless `MODE` bends it.
const STAND_IN: &str = r#"import json, os, subprocess, sys

mode = os.environ.get("MODE", "")
answers = {}
for l in subprocess.run([os.environ["RULEC"], "vectors", "parcel.rule"], capture_output=True, text=True).stdout.splitlines():
    if l.strip():
        v = json.loads(l)
        answers[json.dumps(v["in"], sort_keys=True)] = v["out"]

sys.stdin.readline()
if mode == "ends_at_once":
    sys.stderr.write("ModuleNotFoundError: No module named 'legacy'\n")
    sys.exit(3)
print(json.dumps({"ok": True, "impl": "stand-in"}), flush=True)
n = 0
for line in sys.stdin:
    req = json.loads(line)
    n += 1
    out = dict(answers[json.dumps(req["in"], sort_keys=True)])
    if mode == "ends_at_the_third" and n == 3:
        sys.exit(1)
    if mode == "out_is_a_number":
        print(json.dumps({"id": req["id"], "out": out["fee"]}), flush=True)
    elif mode == "id_is_a_string":
        print(json.dumps({"id": str(req["id"]), "out": out}), flush=True)
    else:
        if mode == "fee_is_a_string":
            out["fee"] = str(out["fee"])
        if mode == "days_is_left_out":
            del out["days"]
        print(json.dumps({"id": req["id"], "out": out}), flush=True)
"#;

/// `rulec verify parcel.rule <flags> --adapter python3 adapter.py` in a directory that holds the
/// two, with the stand-in bent by `mode`: the exit status, stdout and stderr.
fn stand_in(tag: &str, mode: &str, flags: &[&str]) -> Option<(i32, String, String)> {
    if !ready(Need::Python, || have("python3"), "python3 is not here, so the adapter's side of verify was skipped") {
        return None;
    }
    let tmp = TempDir::new(&format!("verify-{tag}"));
    let dir = tmp.path();
    std::fs::write(dir.join("parcel.rule"), PARCEL).unwrap();
    std::fs::write(dir.join("adapter.py"), STAND_IN).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(dir)
        .env("RULEC_LANG", "en")
        .env("RULEC", env!("CARGO_BIN_EXE_rulec"))
        .env("MODE", mode)
        .args(["verify", "parcel.rule"])
        .args(flags)
        .args(["--adapter", "python3", "adapter.py"])
        .output()
        .expect("rulec starts");
    Some((o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned()))
}

/// The stand-in matches a rule that walks a sequence and has two outputs, record for record.
#[test]
fn the_stand_in_matches_exactly() {
    let Some((code, out, err)) = stand_in("faithful", "", &[]) else { return };
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("matched 15 (100.000%)") && out.contains("Counterpart: stand-in"), "{out}");
    assert!(err.is_empty(), "an adapter that answers in time is said nothing of: {err}");
}

/// An adapter that ends where an answer is due is said to have ended, with its exit status and
/// where its own words are. It read "is not JSON: missing value" at the handshake — the adapter
/// had raised on its first line, and the reader was sent to look at JSON — and "went silent" or
/// "Broken pipe" after it (DESIGN §15.306).
#[test]
fn an_adapter_that_ends_is_said_to_have_ended() {
    let Some((code, out, err)) = stand_in("ends-at-once", "ends_at_once", &[]) else { return };
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("The adapter ended before answering the handshake (exit status 3)"), "{err}");
    assert!(err.contains("No module named 'legacy'"), "what the adapter said is passed through: {err}");
    let Some((code, out, err)) = stand_in("ends-later", "ends_at_the_third", &[]) else { return };
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("The adapter ended without answering record 2 (exit status 1)"), "{err}");
}

/// `out` is the object of the outputs by name. A bare value in its place stops the run and says
/// so; it was read as an answer that held nothing, and every record came out a mismatch with no
/// value to show for it. An `id` in quotes is no record's number.
#[test]
fn an_answer_of_another_shape_stops_the_run() {
    let Some((code, out, err)) = stand_in("out-number", "out_is_a_number", &[]) else { return };
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("The `out` of the adapter's answer to record 0 is not an object") && err.contains(r#"{"id": 0, "out": 500}"#), "{err}");
    let Some((code, out, err)) = stand_in("id-string", "id_is_a_string", &[]) else { return };
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("The `id` of the adapter's answer to record 0 is not an integer"), "{err}");
}

/// A value of another type is shown as the JSON it came as. `"500"` where a number goes is a
/// mismatch, and the witness read `rule fee=500 / legacy fee=500`: nothing told the two apart,
/// in the text or in `--format json`.
#[test]
fn a_string_where_a_number_goes_is_shown_in_its_quotes() {
    let Some((code, out, err)) = stand_in("fee-string", "fee_is_a_string", &[]) else { return };
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("matched 0 (0.000%)"), "{out}");
    assert!(out.contains(r#"Example: dest=near, lines=[], weight=1 → rule fee=500 / legacy fee="500""#), "{out}");
    let Some((code, js, err)) = stand_in("fee-string-json", "fee_is_a_string", &["--format", "json"]) else { return };
    assert_eq!(code, 1, "{js}{err}");
    assert!(js.contains(r#""ours":{"fee":500},"theirs":{"fee":"500"}"#), "{js}");
    // The sequence in the witness is the array the adapter was sent, not a string of it.
    assert!(js.contains(r#""witness":{"in":{"dest":"near","lines":[{"quantity":1}],"weight":11}"#), "{js}");
}

/// An output the answer leaves out is a mismatch that says so. The witness used to show the
/// rule's value beside nothing at all.
#[test]
fn an_output_the_answer_leaves_out_is_shown_as_no_answer() {
    let Some((code, out, err)) = stand_in("days-out", "days_is_left_out", &[]) else { return };
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("rule days=1 / legacy days=(no answer)"), "{out}");
    let Some((_, js, _)) = stand_in("days-out-json", "days_is_left_out", &["--format", "json"]) else { return };
    assert!(js.contains(r#""ours":{"days":1},"theirs":{}"#), "{js}");
}

/// The template answers with every output, and names each sequence with its elements' fields.
/// It wrote the first output alone — a rule with two came out of a filled-in template at 0% —
/// and told a rule that only walks a sequence that it had no inputs.
#[test]
fn the_template_names_every_output_and_every_sequence() {
    let tmp = TempDir::new("verify-template");
    std::fs::write(tmp.path().join("parcel.rule"), PARCEL).unwrap();
    let run = |lang: &str| -> String {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .env("RULEC_LANG", "en")
            .current_dir(tmp.path())
            .args(["adapter", "parcel.rule", "--template", lang])
            .output()
            .unwrap();
        assert!(o.status.success(), "{lang}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    let py = run("python");
    assert!(py.contains("# inputs: weight, dest, lines[] (quantity)"), "{py}");
    assert!(py.contains("Every output is required: fee, days."), "{py}");
    assert!(py.contains(r#""out": {"fee": got, "days": got}"#), "{py}");
    let go = run("go");
    assert!(go.contains("Inputs: weight, dest, lines[] (quantity). Every output is required: fee, days."), "{go}");
    assert!(go.contains(r#""out": map[string]any{"fee": got, "days": got}"#), "{go}");
}

/// A stand-in that says nothing where it is told to — at the handshake, or at the record of a
/// number — until the file `said` is there, which is when rulec has spoken of its silence; and
/// that then takes `slow` seconds more over record 0. It gives up after a minute, so a run that
/// is never spoken of ends too.
const SILENT: &str = r#"import json, os, subprocess, sys, time

rulec, rule, said, silent_at, slow = sys.argv[1:6]

def hold():
    end = time.time() + 60
    while not os.path.exists(said):
        if time.time() > end:
            sys.exit(7)
        time.sleep(0.05)

answers = {}
for l in subprocess.run([rulec, "vectors", rule], capture_output=True, text=True).stdout.splitlines():
    if l.strip():
        v = json.loads(l)
        answers[json.dumps(v["in"], sort_keys=True)] = v["out"]

sys.stdin.readline()
if silent_at == "handshake":
    hold()
print(json.dumps({"ok": True, "impl": "stand-in"}), flush=True)
for line in sys.stdin:
    req = json.loads(line)
    if silent_at == str(req["id"]):
        hold()
    if req["id"] == 0:
        time.sleep(float(slow))
    print(json.dumps({"id": req["id"], "out": answers[json.dumps(req["in"], sort_keys=True)]}), flush=True)
"#;

/// `verify` on the parcel rule with the silent stand-in, naming a silence after `quiet`: every
/// sentence that was said of the adapter, and the report the run ended with.
fn waited_for(tag: &str, lang: rulec::i18n::Lang, quiet: u64, silent_at: &str, slow: &str) -> Option<(Vec<String>, rulec::report::Report)> {
    if !ready(Need::Python, || have("python3"), "python3 is not here, so the wait for a silent adapter was skipped") {
        return None;
    }
    let tmp = TempDir::new(&format!("verify-{tag}"));
    let at = |name: &str| tmp.path().join(name).to_string_lossy().into_owned();
    std::fs::write(at("parcel.rule"), PARCEL).unwrap();
    std::fs::write(at("adapter.py"), SILENT).unwrap();
    let adapter: Vec<String> = ["python3", &at("adapter.py"), env!("CARGO_BIN_EXE_rulec"), &at("parcel.rule"), &at("said"), silent_at, slow]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let said = std::sync::Mutex::new(Vec::new());
    let rep = rulec::i18n::with(lang, || {
        let Ok((f, c)) = rulec::prepare(PARCEL, &at("parcel.rule")) else { panic!("the parcel rule is read") };
        let vs = rulec::vectors::generate(&f, &c);
        rulec::verify::run_saying(&f, &c, &adapter, &vs, std::time::Duration::from_secs(quiet), &|sentence| {
            said.lock().unwrap().push(sentence.to_string());
            std::fs::write(at("said"), "").unwrap();
        })
    });
    let rep = rep.unwrap_or_else(|e| panic!("the run goes to its end: {e}"));
    Some((said.into_inner().unwrap(), rep))
}

/// An adapter that leaves a line unanswered is spoken of once, and waited for. rulec sets no
/// limit on an answer, and said nothing either: an adapter that wrote its answers without
/// flushing them and rulec waited for each other without a word, for as long as they were left
/// to (DESIGN §15.310). The stand-in is silent at the handshake, then slow over record 0 for
/// longer than the first silence was: one sentence, and every record compared.
#[test]
fn an_adapter_that_says_nothing_is_spoken_of_once_and_waited_for() {
    let Some((said, rep)) = waited_for("silent-handshake", rulec::i18n::Lang::En, 2, "handshake", "3.5") else { return };
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].starts_with("note: the adapter has not answered the handshake for 2 seconds. rulec goes on waiting, and sets no limit."),
        "{said:?}"
    );
    assert!(said[0].contains("does not flush its output after each line") && said[0].contains("print(..., flush=True)"), "{said:?}");
    assert_eq!((rep.total, rep.agreed), (15, 15), "the run went on to its end");
    // The documents show the sentence as rulec says it, after the 30 seconds it waits.
    assert_eq!(rulec::verify::QUIET.as_secs(), 30);
    let shown = said[0].replace("for 2 seconds", "for 30 seconds");
    for page in [root().join("docs/formats.md"), repo().join("website/rulec/docs/compare.md")] {
        let text = std::fs::read_to_string(&page).unwrap();
        assert!(text.contains(&shown), "{} does not show the sentence as it is said:\n{shown}", page.display());
    }
}

/// The sentence names the record that is unanswered, by the number the adapter was sent.
#[test]
fn the_record_an_adapter_is_silent_at_is_named() {
    let Some((said, rep)) = waited_for("silent-record", rulec::i18n::Lang::En, 8, "1", "0") else { return };
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("note: the adapter has not answered record 1 for 8 seconds."), "{said:?}");
    assert_eq!((rep.total, rep.agreed), (15, 15), "the run went on to its end");
}

/// The sentence is in the language of the run, though another thread builds it.
#[test]
fn the_silence_is_spoken_of_in_japanese_too() {
    let Some((said, rep)) = waited_for("silent-ja", rulec::i18n::Lang::Ja, 2, "handshake", "0") else { return };
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with("注記: アダプタがハンドシェイクに 2 秒答えていません。rulec は時間を切らずに待ち続けます。"), "{said:?}");
    assert!(said[0].contains("一行ごとにフラッシュ"), "{said:?}");
    assert_eq!((rep.total, rep.agreed), (15, 15));
    let shown = said[0].replace("に 2 秒答えていません", "に 30 秒答えていません");
    let page = repo().join("website/rulec/docs-ja/compare.md");
    assert!(std::fs::read_to_string(&page).unwrap().contains(&shown), "{} does not show the sentence as it is said:\n{shown}", page.display());
}
