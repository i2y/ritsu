//! A rule that is one step of a state machine (DESIGN §15.148).
//!
//! The table decides one call; `machine` says what every sequence of calls may and may not do,
//! and `check` proves it. What is held here is the whole of that promise, end to end: a claim
//! that breaks comes back with the shortest sequence of calls that breaks it, the generated
//! code hands the state over the way a caller would, two versions are compared as machines,
//! a log is replayed one case at a time, and the certificate's section is refused when forged.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use ritsu_testkit::{Need, TempDir, need, ready};
use ritsu_testkit::tmp::tmpdir_in;

use rulec::json::Json;

const RULE: &str = "tests/corpus/注文の状態.rule";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").current_dir(root()).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// `rulec`, with `tmp` as rulec's TMPDIR. `rulec test` asks swiftc for its version, and swiftc
/// leaves an empty directory in its TMPDIR almost every time; under the test's directory, it goes
/// with that.
fn rulec_tmp(tmp: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").env("TMPDIR", tmp).current_dir(root()).args(args).output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn objects(out: &str) -> Vec<Json> {
    out.lines().filter(|l| l.starts_with('{')).map(|l| rulec::json::parse(l).expect("JSON として読めない")).collect()
}

fn s<'a>(j: &'a Json, k: &str) -> &'a str {
    j.get(k).and_then(|v| v.as_str()).unwrap_or_default()
}

fn arr<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    match j.get(k) {
        Some(Json::Arr(a)) => a,
        _ => &[],
    }
}

fn int(j: &Json, k: &str) -> i64 {
    j.get(k).and_then(|v| v.as_int()).map(|v| v as i64).unwrap_or(-1)
}

/// A scratch directory of this test's own.
fn scratch(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("machine-{tag}"));
    let d = t.path().to_path_buf();
    (t, d)
}

fn corpus() -> String {
    std::fs::read_to_string(root().join(RULE)).unwrap()
}

/// The corpus rule with one line replaced by others. Panics when the line is not there, so a
/// rule that moved on cannot leave a test that quietly compares the rule with itself.
fn edit(src: &str, line_starts: &str, with: &[&str]) -> String {
    let mut hit = false;
    let mut out = String::new();
    for l in src.lines() {
        if l.starts_with(line_starts) && !hit {
            hit = true;
            for w in with {
                out.push_str(w);
                out.push('\n');
            }
        } else {
            out.push_str(l);
            out.push('\n');
        }
    }
    assert!(hit, "`{line_starts}` で始まる行が無い");
    out
}

/// Everything from the first `scenario` up to `examples` taken out.
fn without_scenarios(src: &str) -> String {
    let i = src.find("\nscenario ").expect("scenario が無い") + 1;
    let j = src.find("\nexamples").expect("examples が無い") + 1;
    format!("{}{}", &src[..i], &src[j..])
}

/// A cancel request after shipment cancels and refunds. It passes check: the refund is still
/// once per case, and nothing reaches 出荷済 after 取消.
fn refund_after_shipment() -> String {
    let v = edit(
        &corpus(),
        "| 出荷済 | 入金, 出荷, 取消依頼 |",
        &["| 出荷済 | 取消依頼   | 取消   | 支払額 | true  |", "| 出荷済 | 入金, 出荷 | 状態   | 0円    | false |"],
    );
    edit(&v, "| 出荷済 | 取消依頼 | 3000円 |", &["| 出荷済 | 取消依頼 | 3000円 | 取消 | 3000円 | true |"])
}

/// Payment moves an order straight to 出荷済, and 入金済 keeps every order it holds. Nothing new
/// reaches 入金済, and an order already there can never finish.
fn strands_paid_orders() -> String {
    let v = edit(&corpus(), "| 受付   | 入金 ", &["| 受付 | 入金 | 出荷済 | 0円 | true |"]);
    let v = edit(&v, "| 入金済 | 出荷 ", &[]);
    let v = edit(&v, "| 入金済 | 取消依頼 ", &[]);
    let v = edit(&v, "| 入金済 | 入金, 配達 ", &["| 入金済 | - | 状態 | 0円 | false |"]);
    without_scenarios(&v)
}

fn write(dir: &Path, name: &str, src: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, src).unwrap();
    p.to_string_lossy().into_owned()
}

#[test]
fn 注文の状態は検査を通り_主張が全部成り立つ() {
    let (c, out) = rulec(&["check", RULE, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    assert!(objects(&out).iter().all(|j| s(j, "severity") != "error"), "{out}");
    let (f, ch) = rulec::prepare(&corpus(), RULE).unwrap_or_else(|_| panic!("{RULE} が検査を通らない"));
    let a = rulec::machine::analyze(&f, &ch, rulec::region::DEFAULT_BUDGET as usize).expect("ステートマシンとして読めない");
    assert_eq!(a.reachable.len(), 5, "受付から五つの状態すべてに着くはず");
}

/// The showcase defect: a payment that arrives after cancellation puts the order back. Every
/// row reads as reasonable on its own; three claims break, and each comes with the shortest
/// sequence of calls that breaks it.
#[test]
fn 一つの欠陥で三つの主張が崩れ_最短の手順が付く() {
    let (c, out) = rulec(&["check", "tests/mutants/m_e126.rule", "--format", "json"]);
    assert_eq!(c, 1);
    let js = objects(&out);
    for (code, calls) in [("E124", 2), ("E126", 3), ("E127", 4)] {
        let d = js.iter().find(|j| s(j, "code") == code).unwrap_or_else(|| panic!("{code} が出ない: {out}"));
        let w = d.get("witness").expect("witness が無い");
        let trace = arr(w, "trace");
        assert_eq!(trace.len(), calls, "{code} の手順の長さ");
        // It starts where a case starts, and every call starts where the one before ended.
        let first = trace[0].get("inputs").unwrap();
        assert_eq!(s(first, "状態"), "受付", "{code}");
        for pair in trace.windows(2) {
            let answered = s(pair[0].get("outputs").unwrap(), "次の状態");
            let next = s(pair[1].get("inputs").unwrap(), "状態");
            assert_eq!(answered, next, "{code}: 答えた状態と次の呼び出しの状態が違う");
        }
        for step in trace {
            assert_eq!(arr(step, "rows").len(), 1, "{code}: どの呼び出しも表 遷移 の一行で答える");
        }
        // The witness's own inputs are the last call's.
        assert_eq!(s(w.get("inputs").unwrap(), "状態"), s(trace[calls - 1].get("inputs").unwrap(), "状態"));
    }
}

/// A scenario that no longer holds says which call, and the calls that led there.
#[test]
fn 手順の例の食い違いは呼び出しの並びつきで出る() {
    let (_, out) = rulec(&["check", "tests/mutants/m_e126.rule", "--format", "json"]);
    let js = objects(&out);
    let d = js
        .iter()
        .find(|j| s(j, "code") == "E107" && s(j, "title").contains("次の状態"))
        .unwrap_or_else(|| panic!("手順の例の E107 が出ない: {out}"));
    let w = d.get("witness").unwrap();
    assert_eq!(arr(w, "trace").len(), 3, "三回目の呼び出しで食い違う");
    assert_eq!(s(w.get("expected").unwrap(), "次の状態"), "取消");
    assert_eq!(s(w.get("outputs").unwrap(), "次の状態"), "入金済");
}

/// A machine whose only output is the next state: the generated function then returns the
/// state itself rather than a record of one field, and every runner has to hand that value on.
/// Ten of them read a field that is not there until this was written.
#[test]
fn 出力が状態だけの機械も生成したコードで回る() {
    let (_tmp, d) = scratch("single");
    let rule = write(
        &d,
        "信号.rule",
        "rule 信号(traffic_light) v1\n\n\
         enum 灯(light) = 赤(red) | 青(green) | 黄(yellow) | 消灯(off)\n\
         enum 合図(cue) = 進め(proceed) | 止まれ(halt) | 切る(cut)\n\n\
         inputs\n  灯(now)   : 灯\n  合図(cue) : 合図\n\n\
         outputs\n  次の灯(next_light) : 灯\n\n\
         table 切替(toggle)\npolicy first\n\
         | 灯 | 合図   | -> 次の灯 |\n\
         | -  | 切る   | 消灯      |\n\
         | 赤 | 進め   | 青        |\n\
         | 青 | 止まれ | 黄        |\n\
         | 黄 | -      | 赤        |\n\
         | -  | -      | 灯        |\n\n\
         machine 信号(signal) over 切替\n  carry   灯 -> 次の灯\n  initial 赤\n  final   消灯\n",
    );
    let (c, out) = rulec(&["check", &rule]);
    assert_eq!(c, 0, "{out}");
    let out_dir = d.join("gen").to_string_lossy().into_owned();
    let (c, out) = rulec(&["gen", &rule, "--out", &out_dir]);
    assert_eq!(c, 0, "{out}");
    assert!(d.join("gen/vectors/traffic_light.traces.jsonl").exists(), "手順のベクタが出ていない");
    if need(Need::Python) {
        let (c, out) = rulec_tmp(&tmpdir_in(Path::new(&out_dir)), &["test", &out_dir, "--format", "json"]);
        assert_eq!(c, 0, "生成したコードが手順で食い違う:\n{out}");
    }
}

/// Two versions compared as machines: the shortest sequence of calls they answer differently,
/// and what the change does to a case already on its way.
#[test]
fn 二つの版の差は呼び出しの並びと処理中の案件で出る() {
    let (_tmp, d) = scratch("diff");
    let v2 = write(&d, "v2.rule", &refund_after_shipment());
    let v3 = write(&d, "v3.rule", &strands_paid_orders());
    for v in [&v2, &v3] {
        let (c, out) = rulec(&["check", v]);
        assert_eq!(c, 0, "{v} が検査を通らない: {out}");
    }
    let (c, out) = rulec(&["diff", RULE, &v2, "--format", "json"]);
    assert_eq!(c, 1, "答えの変わる入力があれば 1: {out}");
    let j = &objects(&out)[0];
    let m = j.get("machine").expect("machine が無い");
    let shortest = arr(m, "shortest");
    assert_eq!(shortest.len(), 3, "入金・出荷・取消依頼の三回: {out}");
    let last = &shortest[2];
    assert_eq!(s(last.get("old").unwrap(), "次の状態"), "出荷済");
    assert_eq!(s(last.get("new").unwrap(), "次の状態"), "取消");
    for call in &shortest[..2] {
        assert_eq!(call.get("old"), call.get("new"), "最後の一回より前は同じ答え");
    }
    assert!(arr(m, "migration").is_empty(), "取り残される状態は無い: {out}");

    let (_, out) = rulec(&["diff", RULE, &v3, "--format", "json"]);
    let m = objects(&out)[0].get("machine").cloned().expect("machine が無い");
    let mig = arr(&m, "migration");
    assert_eq!(mig.len(), 1, "{out}");
    assert_eq!((s(&mig[0], "state"), s(&mig[0], "kind")), ("入金済", "stranded"));
}

/// Records that share a tag are one case's calls, and the case is played again with the state
/// the version itself answered.
#[test]
fn 過去の記録は案件ごとに通し直す() {
    let (_tmp, d) = scratch("replay");
    let log = write(
        &d,
        "log.jsonl",
        r#"{"tag":"order:1","in":{"状態":"受付","出来事":"入金","支払額":3000},"observed":{"次の状態":"入金済","返金額":0,"受理":true}}
{"tag":"order:1","in":{"状態":"入金済","出来事":"出荷","支払額":3000},"observed":{"次の状態":"出荷済","返金額":0,"受理":true}}
{"tag":"order:2","in":{"状態":"受付","出来事":"入金","支払額":5000},"observed":{"次の状態":"入金済","返金額":0,"受理":true}}
{"tag":"order:1","in":{"状態":"出荷済","出来事":"取消依頼","支払額":3000},"observed":{"次の状態":"出荷済","返金額":0,"受理":false}}
{"tag":"order:2","in":{"状態":"入金済","出来事":"取消依頼","支払額":5000},"observed":{"次の状態":"取消","返金額":5000,"受理":true}}
"#,
    );
    let (c, out) = rulec(&["replay", RULE, "--fixtures", &log, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    let cs = objects(&out)[0].get("cases").cloned().expect("cases が無い");
    assert_eq!((int(&cs, "total"), int(&cs, "followed"), int(&cs, "ended")), (2, 2, 1), "{out}");

    let v2 = write(&d, "v2.rule", &refund_after_shipment());
    let (_, out) = rulec(&["diff", RULE, &v2, "--fixtures", &log, "--format", "json"]);
    let cs = objects(&out)[0].get("cases").cloned().expect("cases が無い");
    let parted = arr(&cs, "diverged");
    assert_eq!(parted.len(), 1, "{out}");
    assert_eq!((s(&parted[0], "tag"), int(&parted[0], "line")), ("order:1", 4), "order:1 の三回目で分かれる");
    assert_eq!(int(&cs, "ended"), 2, "新しい版では order:1 も取消で終わる");

    // A case is told by what happens to it first: order:2 parts at its first call, before the
    // record the new version cannot read.
    let v3 = write(&d, "v3.rule", &strands_paid_orders());
    let (_, out) = rulec(&["diff", RULE, &v3, "--fixtures", &log, "--format", "json"]);
    let cs = objects(&out)[0].get("cases").cloned().expect("cases が無い");
    let at: Vec<(String, i64)> = arr(&cs, "diverged").iter().map(|x| (s(x, "tag").to_string(), int(x, "line"))).collect();
    assert_eq!(at, [("order:1".to_string(), 1), ("order:2".to_string(), 3)], "{out}");
    assert!(arr(&cs, "refused").is_empty(), "{out}");
}

#[test]
fn 網羅の基準にステートマシンの遷移が入る() {
    let (c, out) = rulec(&["coverage", RULE, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    let j = &objects(&out)[0];
    let m = arr(j, "criteria").iter().find(|x| s(x, "name") == "machine_transition").cloned().expect("machine_transition が無い");
    assert!(int(&m, "total") > 10, "遷移とその対が数えられていない: {out}");
    assert_eq!(int(&m, "satisfied"), int(&m, "total"), "{out}");
}

/// A decline happens in every world, and the hold that may follow it only when the case holds
/// `hold`. The pair is made in that world, even though the decline is first found in the
/// other one — the traces have to look for it there, as the coverage counts it there.
#[test]
fn 片方の世界でしか続かない遷移の対も手順に入る() {
    let (_tmp, d) = scratch("pair-world");
    let rule = write(
        &d,
        "hold_flow.rule",
        "rule hold_flow v1

enum state = open | authorized | done | void
enum event = decline | confirm | capture | cancel
enum mode = auto | hold

inputs
  state : state
  event : event
  mode  : mode

outputs
  next_state : state

table step
policy unique
| state      | event            | mode | -> next_state |
| open       | decline, capture | -    | state         |
| open       | confirm          | auto | done          |
| open       | confirm          | hold | authorized    |
| open       | cancel           | -    | void          |
| authorized | capture          | -    | done          |
| authorized | cancel           | -    | void          |
| authorized | decline, confirm | -    | state         |
| done, void | -                | -    | state         |

machine flow over step
  carry   state -> next_state
  held    mode
  initial open
  final   done, void
",
    );
    let (c, out) = rulec(&["coverage", &rule, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    let j = &objects(&out)[0];
    let m = arr(j, "criteria").iter().find(|x| s(x, "name") == "machine_transition").cloned().expect("machine_transition が無い");
    assert_eq!(int(&m, "satisfied"), int(&m, "total"), "{out}");
}

#[test]
fn 予算が尽きたら_成り立つとは言わない() {
    let (c, out) = rulec(&["check", RULE, "--budget", "60", "--format", "json"]);
    assert_eq!(c, 1);
    let codes: Vec<String> = objects(&out).iter().map(|j| s(j, "code").to_string()).collect();
    assert_eq!(codes, ["E128"], "{out}");
}

#[test]
fn apiがステートマシンの呼び方を言う() {
    let (c, out) = rulec(&["api", RULE, "--format", "json"]);
    assert_eq!(c, 0);
    let j = rulec::json::parse(&out).unwrap();
    let m = j.get("machine").expect("machine が無い");
    assert_eq!(s(m, "initial"), "受付");
    assert_eq!(arr(m, "final").iter().filter_map(|x| x.as_str()).collect::<Vec<_>>(), ["配達済", "取消"]);
    assert_eq!(s(m.get("carry").unwrap(), "input"), "状態");
    assert_eq!(s(m.get("carry").unwrap(), "output"), "次の状態");
    let py = m.get("constants").and_then(|x| x.get("python")).expect("python の綴りが無い");
    assert_eq!((s(py, "initial"), s(py, "final"), s(py, "is_final")), ("INITIAL", "FINAL", "is_final"));
    let php = m.get("constants").and_then(|x| x.get("php")).unwrap();
    assert_eq!(s(php, "final"), "FINAL_STATES", "PHP では final が予約語");
    // A rule with no machine says so, rather than leaving the key out.
    let (_, out) = rulec(&["api", "tests/corpus/送料.rule", "--format", "json"]);
    assert!(matches!(rulec::json::parse(&out).unwrap().get("machine"), Some(Json::Null)), "machine が null でない");
}

#[test]
fn 承認者の資料に状態の移り方が載る() {
    let (c, md) = rulec(&["doc", RULE]);
    assert_eq!(c, 0);
    assert!(md.contains("```mermaid") && md.contains("stateDiagram"), "状態遷移図が無い:\n{md}");
    for want in ["受付", "配達済", "never", "once"] {
        assert!(md.contains(want), "{want} が載っていない");
    }
    let (_tmp, d) = scratch("doc");
    let out = d.to_string_lossy().into_owned();
    let (c, _) = rulec(&["doc", RULE, "--format", "html", "--out", &out]);
    assert_eq!(c, 0);
    let html = std::fs::read_to_string(d.join("order_state.html")).unwrap();
    assert!(html.contains("<figure class=\"machine\""), "ページに状態の図が無い");
}

// ---------------------------------------------------------------------------------------------
// The certificate's section, and the two programs that read it back.

fn cert() -> String {
    let (c, out) = rulec(&["certificate", RULE]);
    assert_eq!(c, 0);
    out
}

/// Ways to forge the machine's section. Each makes a claim the rows do not bear out.
fn forgeries(cert: &str) -> Vec<(&'static str, String)> {
    let v = vec![
        ("配達済 を落とした到達集合", cert.replace(r#""reach":[0,1,2,3,4]"#, r#""reach":[0,1,2,4]"#)),
        ("行 2 の行き先", cert.replace(r#"{"row":2,"to":4,"#, r#"{"row":2,"to":3,"#)),
        ("出荷済 を終わりにした", cert.replace(r#""finals":[3,4]"#, r#""finals":[2,3,4]"#)),
        ("never の組の印", cert.replace("[4,true]", "[4,false]")),
        ("once の組を一つ落とした", cert.replace(",[4,1]", "")),
        ("終わりへの道の行", cert.replace(r#"{"state":2,"row":7,"#, r#"{"state":2,"row":8,"#)),
    ];
    for (what, forged) in &v {
        assert_ne!(forged, cert, "{what}: 証明書の形が変わっていて、偽れていない");
    }
    v
}

fn feed(cmd: &mut Command, cert: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = cmd.current_dir(root()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("起動できない");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

#[test]
fn ステートマシンの証明書は再検査を通り_偽れば落ちる() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false), "python3 が無い") {
        return;
    }
    let cert = cert();
    let (c, said) = feed(Command::new("python3").args(["tools/recheck.py", "--rule", RULE]), &cert);
    assert_eq!(c, 0, "{said}");
    assert!(said.contains("machine over 遷移") && said.contains("every claim this program states was proved"), "{said}");
    for (what, forged) in forgeries(&cert) {
        let (c, said) = feed(Command::new("python3").args(["tools/recheck.py"]), &forged);
        assert_eq!(c, 1, "{what}: 偽った証明書が通った:\n{said}");
        assert!(said.contains("machine"), "{what}: ステートマシンの検査で落ちていない:\n{said}");
    }
}

#[test]
fn ステートマシンの証明書は証明付きの検査器でも確かめられる() {
    let bin = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    if !ready(Need::Lean, || bin.exists(), "ritsu の proofs/ が build されていない（そこで lake build で作る）") {
        return;
    }
    let cert = cert();
    let (c, said) = feed(Command::new(&bin).args(["--rule", RULE]), &cert);
    assert_eq!(c, 0, "{said}");
    assert!(said.contains("every state reached can still finish"), "{said}");
    for (what, forged) in forgeries(&cert) {
        let (c, said) = feed(&mut Command::new(&bin), &forged);
        assert_eq!(c, 1, "{what}: 偽った証明書が通った:\n{said}");
    }
}

/// The carried pair is in the graph, but not as an edge: an edge is read within one call, and
/// this one crosses to the next.
#[test]
fn グラフは持ち越しを辺にしない() {
    let (c, out) = rulec(&["graph", RULE]);
    assert_eq!(c, 0);
    let j = rulec::json::parse(&out).unwrap();
    let carry = j.get("carry").expect("carry が無い");
    assert_eq!((s(carry, "output"), s(carry, "input")), ("次の状態", "状態"));
    assert!(
        !arr(&j, "edges").iter().any(|e| s(e, "from") == "次の状態" && s(e, "to") == "状態"),
        "持ち越しが辺になっている"
    );
    let (_, out) = rulec(&["graph", "tests/corpus/送料.rule"]);
    assert!(matches!(rulec::json::parse(&out).unwrap().get("carry"), Some(Json::Null)));
}

/// `vectors --out` writes the sequences of calls beside the single calls, as `gen` does.
#[test]
fn ベクタは手順も書き出す() {
    let (_tmp, d) = scratch("vectors");
    let out = d.to_string_lossy().into_owned();
    let (c, said) = rulec(&["vectors", RULE, "--out", &out]);
    assert_eq!(c, 0, "{said}");
    let traces = std::fs::read_to_string(d.join("order_state.traces.jsonl")).expect("手順のベクタが無い");
    let first = rulec::json::parse(traces.lines().next().unwrap()).unwrap();
    assert_eq!(s(&first, "machine"), "meta");
    assert!(traces.lines().any(|l| l.contains(r#""step":"start""#)));
    assert!(traces.lines().any(|l| l.contains(r#""step":"next""#)));
}

// ---------------------------------------------------------------------------------------------
// `held`: the inputs a case holds from its first call to its last (§15.149).

/// Small applications are approved at once; an amendment above the threshold would send one
/// to review. That cannot happen if the amount stays what it was when the case began.
const REVIEW: &str = "rule 申込の審査(review_flow) v1

enum 状態(state) = 受付(received) | 審査中(reviewing) | 小口承認(small_ok) | 承認(approved) | 却下(rejected)
enum 出来事(event) = 申込(apply) | 変更(amend) | 決裁(decide) | 否決(deny)

inputs
  状態(state)    : 状態
  出来事(event)  : 出来事
  申込額(amount) : money[円, incl_tax]  range >=1円 <=1000万円

outputs
  次の状態(next_state) : 状態

table 遷移(step)
policy unique
| 状態     | 出来事           | 申込額   | -> 次の状態 |
| 受付     | 申込             | <=10万円 | 小口承認    |
| 受付     | 申込             | >10万円  | 審査中      |
| 受付     | 変更, 決裁, 否決 | -        | 状態        |
| 審査中   | 決裁             | -        | 承認        |
| 審査中   | 否決             | -        | 却下        |
| 審査中   | 申込, 変更       | -        | 状態        |
| 小口承認 | 変更             | >10万円  | 審査中      |
| 小口承認 | 変更             | <=10万円 | 状態        |
| 小口承認 | 申込, 決裁, 否決 | -        | 状態        |
| 承認     | -                | -        | 状態        |
| 却下     | -                | -        | 状態        |

machine 申込(application) over 遷移
  carry   状態 -> 次の状態
  held    申込額
  initial 受付
  final   承認, 却下, 小口承認
  never   審査中 after 小口承認
";

fn codes_of(src: &str) -> Vec<String> {
    let mut v: Vec<String> = rulec::check_source(src, "r.rule").iter().map(|d| d.code.to_string()).collect();
    v.sort();
    v
}

#[test]
fn heldの入力を変える並びは反例にしない() {
    // Held fixed, the amendment row is one no case takes, and nothing breaks.
    assert_eq!(codes_of(REVIEW), ["W126"]);
    // Free to change, the same table breaks two claims with a sequence no applicant can make.
    let free = REVIEW.replace("  held    申込額\n", "");
    assert_eq!(codes_of(&free), ["E124", "E126"]);
}

#[test]
fn heldの行には入力だけを書ける() {
    for (line, what) in [("  held    次の状態\n", "出力"), ("  held    状態\n", "持ち越す状態"), ("  held    申込額, 申込額\n", "二度")] {
        let src = REVIEW.replace("  held    申込額\n", line);
        assert!(codes_of(&src).contains(&"E056".to_string()), "{what}: {:?}", codes_of(&src));
    }
}

#[test]
fn 手順の例はheldの入力を変えない() {
    let src = corpus().replace(
        "| 入金     | 3000円 | 取消        | 0円    | false |",
        "| 入金     | 5000円 | 取消        | 0円    | false |",
    );
    assert_ne!(src, corpus(), "手順の例の行が見つからない");
    let ds = rulec::check_source(&src, RULE);
    let d = ds.iter().find(|d| d.code == "E055").unwrap_or_else(|| panic!("{:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>()));
    assert!(d.title.contains("支払額"), "{}", d.title);
}

/// A trace is one case, so it passes the same paid amount on every call.
#[test]
fn 手順のベクタはheldの入力を保つ() {
    let (f, c) = rulec::prepare(&corpus(), RULE).unwrap_or_else(|_| panic!("通るはず"));
    let t = rulec::vectors::machine_traces(&f, &c).expect("手順が無い");
    assert!(!t.traces.is_empty());
    for tr in &t.traces {
        let paid: Vec<_> = tr.steps.iter().map(|s| s.get("支払額").cloned()).collect();
        assert!(paid.windows(2).all(|w| w[0] == w[1]), "{}: {paid:?}", tr.why);
    }
}

/// A record that changes a held input is not a call of the same case: the version refuses
/// the case there.
#[test]
fn heldの入力が変わる案件はエラーにする() {
    let (_tmp, d) = scratch("fixed-replay");
    let log = write(
        &d,
        "log.jsonl",
        r#"{"tag":"order:1","in":{"状態":"受付","出来事":"入金","支払額":3000},"observed":{"次の状態":"入金済","返金額":0,"受理":true}}
{"tag":"order:1","in":{"状態":"入金済","出来事":"取消依頼","支払額":5000},"observed":{"次の状態":"取消","返金額":5000,"受理":true}}
"#,
    );
    let (_, out) = rulec(&["replay", RULE, "--fixtures", &log, "--format", "json"]);
    let cs = objects(&out)[0].get("cases").cloned().expect("cases が無い");
    let refused = arr(&cs, "refused");
    assert_eq!(refused.len(), 1, "{out}");
    assert_eq!(int(&refused[0], "line"), 2, "{out}");
}

#[test]
fn apiがheldの入力を言う() {
    let (_, out) = rulec(&["api", RULE, "--format", "json"]);
    let j = rulec::json::parse(&out).unwrap();
    let m = j.get("machine").unwrap();
    assert_eq!(arr(m, "held").iter().filter_map(|x| x.as_str()).collect::<Vec<_>>(), ["支払額"]);
}

/// The certificate lays the claims on the rows once per world, and both re-checkers refuse a
/// world left out, a reach set that leaves out a state the world's rows lead to, and a path
/// made with another world's amount.
#[test]
fn 世界ごとの証明書は再検査を通り_偽れば落ちる() {
    let (_tmp, d) = scratch("worlds");
    let rule = write(&d, "review.rule", REVIEW);
    let (c, cert) = rulec(&["certificate", &rule]);
    assert_eq!(c, 0, "{cert}");
    let j = rulec::json::parse(&cert).unwrap();
    let m = j.get("machine").unwrap();
    assert_eq!(arr(m, "worlds").len(), 5, "申込額の座標の数だけ世界がある");
    assert!(arr(m, "uncertified").is_empty(), "{cert}");
    // The world of an amount above 10万円 reaches 審査中; in the others it never does.
    let forged = vec![
        ("世界の一つを別の世界と重ねた", cert.replacen(r#""at":[0],"reach""#, r#""at":[1],"reach""#, 1)),
        ("到達集合から審査中を落とした", cert.replacen(r#""reach":[0,1,3,4]"#, r#""reach":[0,3,4]"#, 1)),
        // A call on a path of the world of coordinate 3, moved to coordinate 4: a path made
        // with another world's amount.
        ("別の世界の額で呼んだ", cert.replacen(r#""at":[2,1,3]"#, r#""at":[2,1,4]"#, 1)),
    ];
    let python = ready(Need::Python, || Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false), "python3 が無いので Python の再検査を飛ばした");
    let lean = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    let lean = if ready(Need::Lean, || lean.exists(), "ritsu の proofs/ が build されていない（そこで lake build で作る）") { lean } else { PathBuf::new() };
    if python {
        let (c, said) = feed(Command::new("python3").args(["tools/recheck.py", "--rule", &rule]), &cert);
        assert_eq!(c, 0, "{said}");
        assert!(said.contains("in 5 worlds"), "{said}");
    }
    if lean.exists() {
        let (c, said) = feed(Command::new(&lean).args(["--rule", &rule]), &cert);
        assert_eq!(c, 0, "{said}");
    }
    for (what, f) in forged {
        assert_ne!(f, cert, "{what}: 偽れていない");
        if python {
            let (c, said) = feed(Command::new("python3").args(["tools/recheck.py"]), &f);
            assert_eq!(c, 1, "{what}: Python が通した:\n{said}");
        }
        if lean.exists() {
            let (c, said) = feed(&mut Command::new(&lean), &f);
            assert_eq!(c, 1, "{what}: Lean が通した:\n{said}");
        }
    }
}

/// The review rule in every language that runs here. Its `final` line lists the states in an
/// order that is not the enum's, which the constants every runner prints first are held to.
#[test]
fn heldのある機械も生成したコードで回る() {
    if !need(Need::Python) {
        return;
    }
    let (_tmp, d) = scratch("review-gen");
    let rule = write(&d, "review.rule", REVIEW);
    let out_dir = d.join("gen").to_string_lossy().into_owned();
    let (c, out) = rulec(&["gen", &rule, "--out", &out_dir]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec_tmp(&tmpdir_in(Path::new(&out_dir)), &["test", &out_dir, "--format", "json"]);
    assert_eq!(c, 0, "生成したコードが食い違う:\n{out}");
}

// ---------------------------------------------------------------------------------------------
// Values carried along with the state (§15.217): a number with a range, a truth value, another
// enum, each on a `carry` line of its own.

/// The corpus rules that carry more than their state, in English and in Japanese.
const MORE: &[&str] = &[
    "tests/corpus/retry_attempts.rule",
    "tests/corpus/grace_period.rule",
    "tests/corpus/one_time_perk.rule",
    "tests/corpus/再試行の回数.rule",
    "tests/corpus/猶予の残り日数.rule",
    "tests/corpus/一度だけの特典.rule",
];

fn rulec_en(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(root()).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The codes `check` prints for a file, in order.
fn printed(out: &str) -> Vec<String> {
    objects(out).iter().map(|j| s(j, "code").to_string()).filter(|c| !c.is_empty()).collect()
}

/// The standing between a marketplace and a seller, with the days left of a grace period or a
/// probation carried along with it. A grace period always starts at sixteen days, so the row for
/// a night of grace at zero days is one no case reaches.
const STANDING: &str = "rule seller_standing v1

enum standing = fresh | dispute | grace | probation | good | trusted
enum event = first_sale | warn | review | promote | revoke | night

inputs
  standing  : standing
  event     : event
  days_left : number  range >=0 <=16

outputs
  next_standing  : standing
  next_days_left : number  round down(1)

derive one_less : number = days_left - 1  range >=-1 <=15

table step
policy unique
| standing                        | event      | days_left | -> next_standing | next_days_left |
| fresh                           | first_sale | -         | dispute          | days_left      |
| not: fresh                      | first_sale | -         | standing         | days_left      |
| dispute                         | warn       | -         | grace            | 16             |
| not: dispute                    | warn       | -         | standing         | days_left      |
| dispute, grace                  | review     | -         | probation        | 16             |
| fresh, probation, good, trusted | review     | -         | standing         | days_left      |
| not: trusted                    | promote    | -         | trusted          | days_left      |
| trusted                         | promote    | -         | standing         | days_left      |
| fresh, dispute                  | revoke     | -         | standing         | days_left      |
| grace, probation, good          | revoke     | -         | dispute          | 16             |
| trusted                         | revoke     | -         | probation        | 16             |
| fresh, dispute, good, trusted   | night      | -         | standing         | days_left      |
| grace                           | night      | 1         | dispute          | 0              |
| grace                           | night      | >=2       | grace            | one_less       |
| grace                           | night      | 0         | grace            | one_less       |
| probation                       | night      | <=1       | good             | 0              |
| probation                       | night      | >=2       | probation        | one_less       |

machine seller over step
  carry   standing -> next_standing
  carry   days_left -> next_days_left
  initial fresh
  initial days_left 0
  never   fresh after dispute, grace, probation, good, trusted
  always  days_left >=1 <=16 when grace, probation
";

#[test]
fn 数と真偽を持ち越すコーパスは検査を通り_主張が全部成り立つ() {
    for rel in MORE {
        let (c, out) = rulec(&["check", rel, "--format", "json"]);
        assert_eq!(c, 0, "{rel}\n{out}");
        let codes = printed(&out);
        assert!(codes.iter().all(|c| !c.starts_with('E') && !c.starts_with("W12")), "{rel}: {codes:?}");
        let src = std::fs::read_to_string(root().join(rel)).unwrap();
        let (f, ch) = rulec::prepare(&src, rel).unwrap_or_else(|_| panic!("{rel} が検査を通らない"));
        let a = rulec::machine::analyze(&f, &ch, rulec::region::DEFAULT_BUDGET as usize).expect("ステートマシンとして読めない");
        assert!(a.whole, "{rel}: 値を一つずつ調べる大きさのはず");
        assert!(a.always_claims.iter().all(|v| v.holds()) && a.range_claims.iter().all(|(_, v)| v.holds()), "{rel}");
    }
}

/// The seller's standing, walked whole: the row for a night of grace at zero days is W126 — a
/// case reaches grace, never with zero days left — and the `always` line holds. A line that says
/// a dispute always has zero days left breaks: a grace period revoked sends the case back to a
/// dispute with sixteen days set, three calls from the start.
#[test]
fn 猶予の残りは届かない行をw126で言い_alwaysの成立と破れを言う() {
    let (_tmp, d) = scratch("standing");
    let rule = write(&d, "standing.rule", STANDING);
    let (c, out) = rulec_en(&["check", &rule, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    assert_eq!(printed(&out), ["W126"], "{out}");
    let w = &objects(&out)[0];
    assert_eq!(arr(w, "rows").iter().map(|r| int(r, "row")).collect::<Vec<_>>(), [15], "{out}");
    assert!(out.contains("never with the days_left this row needs") && out.contains("days_left 1..16"), "{out}");

    let broken = write(&d, "standing2.rule", &STANDING.replace("  always  days_left >=1 <=16 when grace, probation\n", "  always  days_left >=1 <=16 when grace, probation\n  always  days_left 0 when dispute\n"));
    let (c, out) = rulec_en(&["check", &broken, "--format", "json"]);
    assert_eq!(c, 1, "{out}");
    let js = objects(&out);
    let e = js.iter().find(|j| s(j, "code") == "E135").unwrap_or_else(|| panic!("E135 が出ない: {out}"));
    let trace = arr(e.get("witness").unwrap(), "trace");
    assert_eq!(trace.len(), 3, "{out}");
    let last = trace[2].get("outputs").unwrap();
    assert_eq!((s(last, "next_standing"), int(last, "next_days_left")), ("dispute", 16), "{out}");
    // Each call is made with the values the call before answered.
    for pair in trace.windows(2) {
        let (answered, next) = (pair[0].get("outputs").unwrap(), pair[1].get("inputs").unwrap());
        assert_eq!(int(answered, "next_days_left"), int(next, "days_left"));
        assert_eq!(s(answered, "next_standing"), s(next, "standing"));
    }
    assert_eq!(printed(&out).iter().filter(|c| c.starts_with('E')).count(), 1, "{out}");
}

/// A campaign that carries two marks as truth values, so that the welcome and the reminder go out
/// once each. Left without the mark, the welcome goes out on every visit, and `once` breaks two
/// calls from the start.
#[test]
fn 一度だけの印は真偽で持ち越し_印が無ければonceが破れる() {
    let rule = "rule campaign v1

enum phase = running | converted | lost
enum happening = visit | day_starts | purchase | churn
enum reaction = nothing | welcome | reminder | thanks | farewell

inputs
  phase    : phase
  event    : happening
  day      : number  range >=1 <=30
  welcomed : bool
  reminded : bool

outputs
  next_phase    : phase
  next_welcomed : bool
  next_reminded : bool
  reaction      : reaction

table react
policy first
| phase   | event      | day | welcomed | reminded | -> next_phase | next_welcomed | next_reminded | reaction |
| running | visit      | -   | false    | -        | phase         | true          | reminded      | welcome  |
| running | visit      | -   | true     | -        | phase         | welcomed      | reminded      | nothing  |
| running | day_starts | 22  | -        | false    | phase         | welcomed      | true          | reminder |
| running | day_starts | -   | -        | -        | phase         | welcomed      | reminded      | nothing  |
| running | purchase   | -   | -        | -        | converted     | welcomed      | reminded      | thanks   |
| running | churn      | -   | -        | -        | lost          | welcomed      | reminded      | farewell |
| -       | -          | -   | -        | -        | phase         | welcomed      | reminded      | nothing  |

machine campaign over react
  carry   phase -> next_phase
  carry   welcomed -> next_welcomed
  carry   reminded -> next_reminded
  initial running
  initial welcomed false
  initial reminded false
  final   converted, lost
  never   converted after lost
  once    reaction welcome
  once    reaction reminder
  always  welcomed false and reminded false when running
";
    let (_tmp, d) = scratch("campaign");
    let ok = write(&d, "campaign.rule", &rule.replace("  always  welcomed false and reminded false when running\n", ""));
    let (c, out) = rulec_en(&["check", &ok, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    assert!(printed(&out).iter().all(|c| !c.starts_with('E') && !c.starts_with("W12")), "{out}");

    // `and` joins two tests on one line; the line is broken the first time a visit welcomes.
    let both = write(&d, "both.rule", rule);
    let (_, out) = rulec_en(&["check", &both, "--format", "json"]);
    let e = objects(&out).into_iter().find(|j| s(j, "code") == "E135").unwrap_or_else(|| panic!("E135 が出ない: {out}"));
    assert_eq!(arr(e.get("witness").unwrap(), "trace").len(), 1, "{out}");

    let every = write(&d, "every.rule", &rule.replace("| running | visit      | -   | true     | -        | phase         | welcomed      | reminded      | nothing  |", "| running | visit      | -   | true     | -        | phase         | welcomed      | reminded      | welcome  |").replace("  always  welcomed false and reminded false when running\n", ""));
    let (c, out) = rulec_en(&["check", &every, "--format", "json"]);
    assert_eq!(c, 1, "{out}");
    let e = objects(&out).into_iter().find(|j| s(j, "code") == "E127").unwrap_or_else(|| panic!("E127 が出ない: {out}"));
    let trace = arr(e.get("witness").unwrap(), "trace");
    assert_eq!(trace.len(), 2, "{out}");
    assert!(trace.iter().all(|t| s(t.get("outputs").unwrap(), "reaction") == "welcome"), "{out}");
}

/// Ten million values one by one are past the budget, so the count is walked in intervals. A
/// claim that holds there is said to hold; one the intervals break but no sequence of calls that
/// really happens does — an even count taken down two at a time never shows 1 — is W127, with
/// how far the calls that really happen were followed.
#[test]
fn 区間にまとめた探索は成り立つ主張を示し_作り直せない反例をw127で言う() {
    let rule = "rule even_count v1

enum phase = running | done
enum tick = step | stop

inputs
  phase : phase
  tick  : tick
  left  : number  range >=0 <=10_000_000

outputs
  next_phase : phase
  next_left  : number  round down(1)

define two_less : number = left - 2

table step
policy unique
| phase   | tick | left | -> next_phase | next_left |
| running | step | >=2  | phase         | two_less  |
| running | step | <=1  | done          | left      |
| running | stop | -    | done          | left      |
| done    | -    | -    | phase         | left      |

machine count over step
  carry   phase -> next_phase
  carry   left -> next_left
  initial running
  initial left 10_000_000
  never   running after done
  always  left >=0
  always  left not: 1
";
    let (_tmp, d) = scratch("intervals");
    let p = write(&d, "even.rule", rule);
    let (c, out) = rulec_en(&["check", &p, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    let src = std::fs::read_to_string(&p).unwrap();
    let (f, ch) = rulec::prepare(&src, &p).unwrap();
    let a = rulec::machine::analyze(&f, &ch, rulec::region::DEFAULT_BUDGET as usize).unwrap();
    assert!(!a.whole, "区間にまとめて調べるはず");
    assert!(a.never_claims[0].holds() && a.always_claims[0].holds() && a.range_claims.iter().all(|(_, v)| v.holds()), "区間の上で成り立つ主張");
    assert!(matches!(a.always_claims[1], rulec::machine::Verdict::Undecided(_)), "作り直せない反例は決めきれないとする");
    let w = objects(&out).into_iter().find(|j| s(j, "code") == "W127").unwrap_or_else(|| panic!("W127 が出ない: {out}"));
    let notes: Vec<String> = arr(&w, "notes").iter().filter_map(|n| n.as_str().map(String::from)).collect();
    assert!(notes.iter().any(|n| n.contains("could not be made again from calls that really happen") && n.contains("followed to")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("raise `--budget`")), "{notes:?}");
    assert_eq!(printed(&out), ["W127"], "{out}");
}

/// Every kind of value carried at once — an amount that starts below zero, a rate, another enum,
/// a truth value — through the generated code of every language there is a toolchain for, each
/// call passed what the language itself answered to the call before.
#[test]
fn 持ち越す値も生成したコードで手順のとおりに回る() {
    let (_tmp, d) = scratch("kinds");
    let rule = write(
        &d,
        "account.rule",
        "rule account v1

enum status = open | frozen | closed
enum tier = basic | silver | gold
enum event = deposit | withdraw | freeze | close

inputs
  status  : status
  event   : event
  balance : money[JPY]  range >=-5000JPY <=100000JPY
  bonus   : rate[step 1%]  range >=0% <=10%
  level   : tier
  warned  : bool
  amount  : money[JPY]  range >=0JPY <=1000JPY

outputs
  next_status  : status
  next_balance : money[JPY]  round down(1JPY)
  next_bonus   : rate[step 1%]  round down(1%)
  next_level   : tier
  next_warned  : bool

derive added : money[JPY] = balance + amount  range >=-5000JPY <=101000JPY
derive taken : money[JPY] = balance - amount  range >=-6000JPY <=100000JPY

table step
policy first
| status | event    | added       | taken      | -> next_status | next_balance | next_bonus | next_level | next_warned |
| open   | deposit  | <=100000JPY | -          | status         | added        | bonus      | silver     | warned      |
| open   | deposit  | >100000JPY  | -          | status         | balance      | bonus      | level      | true        |
| open   | withdraw | -           | >=-5000JPY | status         | taken        | 1%         | level      | true        |
| open   | withdraw | -           | <-5000JPY  | status         | balance      | bonus      | level      | true        |
| open   | freeze   | -           | -          | frozen         | balance      | 0%         | level      | warned      |
| open   | close    | -           | -          | closed         | 0JPY         | 0%         | basic      | false       |
| -      | -        | -           | -          | status         | balance      | bonus      | level      | warned      |

machine account over step
  carry   status -> next_status
  carry   balance -> next_balance
  carry   bonus -> next_bonus
  carry   level -> next_level
  carry   warned -> next_warned
  initial open
  initial balance -5000JPY
  initial bonus 5%
  initial level gold
  initial warned false
  final   closed
",
    );
    let out_dir = d.join("gen").to_string_lossy().into_owned();
    let (c, out) = rulec(&["gen", &rule, "--out", &out_dir]);
    assert_eq!(c, 0, "{out}");
    let traces = std::fs::read_to_string(d.join("gen/vectors/account.traces.jsonl")).unwrap();
    assert!(traces.lines().next().unwrap().contains(r#""more":[{"in":"balance","out":"next_balance"}"#), "{traces}");
    assert!(traces.contains(r#""carried":{"balance":-5000,"bonus":5,"level":"gold","warned":false}"#), "{traces}");
    let py = std::fs::read_to_string(d.join("gen/python/account.py")).unwrap();
    for want in ["INITIAL_BALANCE: JPY = JPY(-5000)", "INITIAL_BONUS: Rate = Rate(5)", "INITIAL_LEVEL: Tier = Tier.GOLD", "INITIAL_WARNED: bool = False"] {
        assert!(py.contains(want), "{want}");
    }
    if need(Need::Python) {
        let (c, out) = rulec_tmp(&tmpdir_in(Path::new(&out_dir)), &["test", &out_dir, "--format", "json"]);
        assert_eq!(c, 0, "生成したコードが手順で食い違う:\n{out}");
    }
}

/// The certificate lays the carried values on the rows (§15.217): the places a case can be, closed
/// under the rows' moves, and the claims over them. Both programs re-check it and refuse it forged;
/// the one with the proofs proves the claims (§15.232).
#[test]
fn 持ち越す値の証明書は再検査を通り_偽れば落ちる() {
    const GRACE: &str = "tests/corpus/grace_period.rule";
    let (c, cert) = rulec(&["certificate", GRACE]);
    assert_eq!(c, 0);
    let forged = vec![
        ("場所を一つ落とした", cert.replace("[1,[2]],[1,[1]],[2,[0]]]", "[1,[1]],[2,[0]]]")),
        ("行 1 が書く値", cert.replacen(r#"{"row":1,"to":7,"#, r#"{"row":1,"to":6,"#, 1)),
        ("always の境を狭めた", cert.replacen(r#"{"op":"<=","value":"7"}]}}],"holds":true"#, r#"{"op":"<=","value":"6"}]}}],"holds":true"#, 1)),
        ("始まりの場所", cert.replacen(r#""start":[0]"#, r#""start":[1]"#, 1)),
        ("範囲", cert.replacen(r#""range":["0","7"]"#, r#""range":["0","8"]"#, 1)),
    ];
    for (what, f) in &forged {
        assert_ne!(f, &cert, "{what}: 証明書の形が変わっていて、偽れていない");
    }
    if ready(Need::Python, || Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false), "python3 が無い") {
        let (c, said) = feed(Command::new("python3").args(["tools/recheck.py", "--rule", GRACE]), &cert);
        assert_eq!(c, 0, "{said}");
        assert!(said.contains("carried values over step: 9 places a case can be, 7 claims laid on the rows"), "{said}");
        for (what, f) in &forged {
            let (c, said) = feed(Command::new("python3").args(["tools/recheck.py"]), f);
            assert_eq!(c, 1, "{what}: 偽った証明書が通った:\n{said}");
            assert!(said.contains("carried values"), "{what}: 持ち越す値の検査で落ちていない:\n{said}");
        }
    }
    let bin = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    if ready(Need::Lean, || bin.exists(), "ritsu の proofs/ が build されていない（そこで lake build で作る）") {
        let (c, said) = feed(Command::new(&bin).args(["--rule", GRACE]), &cert);
        assert_eq!(c, 0, "{said}");
        assert!(said.contains("carried values over step: 9 places a case can be"), "{said}");
        assert!(!said.contains("besides its state"), "{said}");
        for (what, f) in &forged {
            let (c, said) = feed(Command::new(&bin).args(["--rule", GRACE]), f);
            assert_eq!(c, 1, "{what}: 偽った証明書が Lean で通った:\n{said}");
            assert!(said.contains("carried values"), "{what}: 持ち越す値の検査で落ちていない:\n{said}");
        }
    }
}

#[test]
fn apiが持ち越す値の定数を言う() {
    let (c, out) = rulec(&["api", "tests/corpus/grace_period.rule", "--format", "json"]);
    assert_eq!(c, 0);
    let j = rulec::json::parse(&out).unwrap();
    let m = j.get("machine").expect("machine が無い");
    let more = arr(m, "carried");
    assert_eq!(more.len(), 1, "{out}");
    assert_eq!((s(&more[0], "input"), s(&more[0], "output"), s(&more[0], "type")), ("days_left", "next_days_left", "number"));
    assert_eq!(int(&more[0], "initial"), 0);
    let k = more[0].get("constants").unwrap();
    assert_eq!((s(k, "python"), s(k, "go"), s(k, "swift"), s(k, "php")), ("INITIAL_DAYS_LEFT", "InitialDaysLeft", "initialDaysLeft", "INITIAL_DAYS_LEFT"));
    assert_eq!(arr(m, "always").len(), 2, "{out}");
}

#[test]
fn 資料に状態ごとの持ち越す値が載る() {
    let (c, md) = rulec_en(&["doc", "tests/corpus/grace_period.rule"]);
    assert_eq!(c, 0);
    for want in [
        "| days_left → next_days_left | number | >=0 <=7 | 0 |",
        "note right of s1 : days_left 1..7",
        "### What each state carries",
        "| in_grace | 1..7 |",
        "`always days_left >=1 <=7 when in_grace`: whenever a case is in in_grace, days_left >=1 <=7.",
        "Every value next_days_left answers lies within the range of days_left.",
    ] {
        assert!(md.contains(want), "{want} が載っていない:\n{md}");
    }
    let (_tmp, d) = scratch("doc-more");
    let out = d.to_string_lossy().into_owned();
    let (c, _) = rulec(&["doc", "tests/corpus/grace_period.rule", "--format", "html", "--out", &out]);
    assert_eq!(c, 0);
    let html = std::fs::read_to_string(d.join("grace_period.html")).unwrap();
    assert!(html.contains("days_left 1..7</text>"), "図に状態ごとの値が無い");
    assert!(html.contains(r#""more":[{"in":"days_left","out":"next_days_left"}]"#) && html.contains("const stepAlone = machineStep;"), "続けるボタンが持ち越す値を戻さない");
}

/// The shapes a carried value is written in (E072), and a scenario that writes one down (E055).
#[test]
fn 持ち越す値の形の誤りはe072で言う() {
    let base = std::fs::read_to_string(root().join("tests/corpus/retry_attempts.rule")).unwrap();
    let (_tmp, d) = scratch("e072");
    let cases: Vec<(&str, String, &str)> = vec![
        ("始まりが無い", base.replace("  initial attempts 0\n", ""), "E072"),
        ("範囲の外の始まり", base.replace("  initial attempts 0\n", "  initial attempts 4\n"), "E072"),
        ("持ち越さない入力の始まり", base.replace("  initial attempts 0\n", "  initial attempts 0\n  initial outcome declined\n"), "E072"),
        ("状態の始まりを二語で", base.replace("  initial pending\n", "  initial status pending\n"), "E072"),
        ("範囲の無い数", base.replace("attempts : number  range >=0 <=3", "attempts : number"), "E072"),
        ("状態を always に", base.replace("  always  attempts >=1 when collected\n", "  always  status pending\n"), "E072"),
        ("持ち越さない値を always に", base.replace("  always  attempts >=1 when collected\n", "  always  outcome declined\n"), "E072"),
        ("always の when の後が空", base.replace("  always  attempts >=1 when collected\n", "  always  attempts >=1 when\n"), "E072"),
        ("二度持ち越す", base.replace("  carry   attempts -> next_attempts\n", "  carry   attempts -> next_attempts\n  carry   attempts -> next_attempts\n"), "E072"),
        ("型の違う組", base.replace("  carry   attempts -> next_attempts\n", "  carry   attempts -> next_status\n"), "E051"),
        ("手順の例に持ち越す値の列", base.replace("| outcome  | -> next_status | next_attempts |\n| declined | pending        | 1             |\n| approved | collected      | 2             |", "| outcome  | attempts | -> next_status | next_attempts |\n| declined | 0        | pending        | 1             |\n| approved | 1        | collected      | 2             |"), "E055"),
        ("持ち越す値を held に", base.replace("  initial pending\n", "  held    attempts\n  initial pending\n"), "E056"),
        ("持ち越す値を once に", base.replace("  always  attempts >=1 when collected\n", "  once    next_attempts 3\n"), "E053"),
    ];
    for (what, src, code) in cases {
        assert_ne!(src, base, "{what}: 書き換えが当たっていない");
        let p = write(&d, "case.rule", &src);
        let (c, out) = rulec(&["check", &p, "--format", "json"]);
        assert_eq!(c, 1, "{what}:\n{out}");
        assert!(printed(&out).iter().any(|x| x == code), "{what}: {code} が出ない:\n{out}");
    }
}

/// Records that share a tag are one case: each later call is passed what the version answered
/// for the carried values too, and a record that says otherwise is where the case parts.
#[test]
fn 過去の記録は持ち越す値も通し直す() {
    let (_tmp, d) = scratch("replay-more");
    let log = write(
        &d,
        "log.jsonl",
        r#"{"tag":"s:1","in":{"status":"active","event":"payment_missed","days_left":0},"observed":{"next_status":"in_grace","next_days_left":7}}
{"tag":"s:2","in":{"status":"active","event":"payment_missed","days_left":0},"observed":{"next_status":"in_grace","next_days_left":7}}
{"tag":"s:1","in":{"status":"in_grace","event":"day_passed","days_left":7},"observed":{"next_status":"in_grace","next_days_left":6}}
{"tag":"s:2","in":{"status":"in_grace","event":"payment_received","days_left":7},"observed":{"next_status":"active","next_days_left":0}}
{"tag":"s:1","in":{"status":"in_grace","event":"day_passed","days_left":5},"observed":{"next_status":"in_grace","next_days_left":4}}
"#,
    );
    let (c, out) = rulec(&["replay", "tests/corpus/grace_period.rule", "--fixtures", &log, "--format", "json"]);
    assert_eq!(c, 0, "{out}");
    let cs = objects(&out)[0].get("cases").cloned().expect("cases が無い");
    assert_eq!((int(&cs, "total"), int(&cs, "followed")), (2, 1), "{out}");
    let parted = arr(&cs, "diverged");
    assert_eq!(parted.len(), 1, "{out}");
    assert_eq!((s(&parted[0], "tag"), int(&parted[0], "line")), ("s:1", 5), "s:1 は三回目で、記録の残り日数が答えと違う");
}

/// The runners `rulec test` drives a machine through, for the forms the example game met (§15.238):
/// a machine whose state is an enum read from an `enums` file with display names (`door`), one
/// whose enums are its own with display names (`lamp`), and one whose state comes from an `enums`
/// file without them (`gate`). Before, the NumPy runner of a machine had no `--labels` branch, the
/// TypeScript and JavaScript runners imported the state's parser twice — and from the rule's module,
/// which does not export it — and the PHP runner named the state's enum in the rule's namespace.
/// Every comparison runs where its toolchain is installed, and none of them fails.
#[test]
fn enumsのファイルから状態を読む機械も全部のrunnerで走る() {
    if !need(Need::Python) {
        return;
    }
    let (_tmp, d) = scratch("runners");
    std::fs::create_dir_all(d.join("shared")).unwrap();
    write(&d, "shared/doors.rule", "enums doors v1\ndescription \"The states of a door, and of a gate\"\n\nenum door = open \"Open\" | shut \"Shut\"\nenum gate = up | down\n");
    let door = write(
        &d,
        "door.rule",
        "rule door_step v1\ndescription \"A door that opens and shuts, its state read from an enums file\"\n\nimport enum door from \"shared/doors.rule\"\n\n\
         enum push = pull \"Pull\" | slam \"Slam\"\n\ninputs\n  state : door\n  push  : push\n\noutputs\n  next_state : door\n\n\
         table step\npolicy unique\n| state | push | -> next_state |\n| -     | pull | open          |\n| -     | slam | shut          |\n\n\
         machine doorway over step\n  carry   state -> next_state\n  initial shut\n",
    );
    let lamp = write(
        &d,
        "lamp.rule",
        "rule lamp v1\ndescription \"A lamp that is lit and put out, its states declared here\"\n\nenum light = lit \"Lit\" | dark \"Dark\"\n\nenum press = on \"On\" | off \"Off\"\n\n\
         inputs\n  state : light\n  press : press\n\noutputs\n  next_state : light\n\n\
         table step\npolicy unique\n| state | press | -> next_state |\n| -     | on    | lit           |\n| -     | off   | dark          |\n\n\
         machine switch over step\n  carry   state -> next_state\n  initial dark\n",
    );
    let gate = write(
        &d,
        "gate.rule",
        "rule gate_step v1\ndescription \"A gate raised and lowered, its state read from an enums file with no display names\"\n\nimport enum gate from \"shared/doors.rule\"\n\n\
         inputs\n  state : gate\n  raise : bool\n\noutputs\n  next_state : gate\n\n\
         table step\npolicy unique\n| state | raise | -> next_state |\n| -     | true  | up            |\n| -     | false | down          |\n\n\
         machine barrier over step\n  carry   state -> next_state\n  initial down\n",
    );
    let out_dir = d.join("gen").to_string_lossy().into_owned();
    let (c, out) = rulec(&["gen", &door, &lamp, &gate, "--out", &out_dir]);
    assert_eq!(c, 0, "{out}");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmpdir_in(Path::new(&out_dir)))
        .args(["test", &out_dir])
        .output()
        .expect("rulec を起動できない");
    let said = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "{said}");
    assert!(!said.contains("FAIL"), "{said}");
    let have = |cmd: &str, arg: &str| Command::new(cmd).arg(arg).output().is_ok_and(|o| o.status.success());
    let numpy = Command::new("python3").args(["-c", "import numpy"]).output().is_ok_and(|o| o.status.success());
    for (lang, here) in [("NumPy, labels", numpy), ("TypeScript", have("node", "--version")), ("JavaScript", have("node", "--version")), ("PHP", have("php", "--version")), ("Go", have("go", "version"))] {
        if !here {
            eprintln!("SKIP {lang}: the toolchain is not installed");
            continue;
        }
        for rule in ["door_step", "lamp", "gate_step"] {
            if lang == "NumPy, labels" && rule == "gate_step" {
                continue; // no display names to ask for
            }
            assert!(said.lines().any(|l| l.starts_with(&format!("ok    {rule} ({lang})"))), "{rule} ({lang}) did not run:\n{said}");
        }
    }
}
