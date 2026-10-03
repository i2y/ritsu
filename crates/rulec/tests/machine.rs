//! A rule that is one step of a state machine (DESIGN §15.148).
//!
//! The table decides one call; `machine` says what every sequence of calls may and may not do,
//! and `check` proves it. What is held here is the whole of that promise, end to end: a claim
//! that breaks comes back with the shortest sequence of calls that breaks it, the generated
//! code hands the state over the way a caller would, two versions are compared as machines,
//! a log is replayed one case at a time, and the certificate's section is refused when forged.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rulec::json::Json;

const RULE: &str = "tests/corpus/注文の状態.rule";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(args).output().expect("rulec を起動できない");
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
fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-machine-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
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
    let d = scratch("single");
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
    let (c, out) = rulec(&["test", &out_dir, "--format", "json"]);
    assert_eq!(c, 0, "生成したコードが手順で食い違う:\n{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Two versions compared as machines: the shortest sequence of calls they answer differently,
/// and what the change does to a case already on its way.
#[test]
fn 二つの版の差は呼び出しの並びと処理中の案件で出る() {
    let d = scratch("diff");
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
    let _ = std::fs::remove_dir_all(&d);
}

/// Records that share a tag are one case's calls, and the case is played again with the state
/// the version itself answered.
#[test]
fn 過去の記録は案件ごとに通し直す() {
    let d = scratch("replay");
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
    let _ = std::fs::remove_dir_all(&d);
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
    let d = scratch("pair-world");
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
    let _ = std::fs::remove_dir_all(&d);
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
    let d = scratch("doc");
    let out = d.to_string_lossy().into_owned();
    let (c, _) = rulec(&["doc", RULE, "--format", "html", "--out", &out]);
    assert_eq!(c, 0);
    let html = std::fs::read_to_string(d.join("order_state.html")).unwrap();
    assert!(html.contains("<figure class=\"machine\""), "ページに状態の図が無い");
    let _ = std::fs::remove_dir_all(&d);
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
    if !Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("skip: python3 が無い");
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
    let bin = root().join("proofs/.lake/build/bin/rulec-recheck");
    if !bin.exists() {
        eprintln!("skip: proofs/ が build されていない（lake build で作る）");
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
    let d = scratch("vectors");
    let out = d.to_string_lossy().into_owned();
    let (c, said) = rulec(&["vectors", RULE, "--out", &out]);
    assert_eq!(c, 0, "{said}");
    let traces = std::fs::read_to_string(d.join("order_state.traces.jsonl")).expect("手順のベクタが無い");
    let first = rulec::json::parse(traces.lines().next().unwrap()).unwrap();
    assert_eq!(s(&first, "machine"), "meta");
    assert!(traces.lines().any(|l| l.contains(r#""step":"start""#)));
    assert!(traces.lines().any(|l| l.contains(r#""step":"next""#)));
    let _ = std::fs::remove_dir_all(&d);
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
fn heldの行は入力だけを名指す() {
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
fn heldの入力が変わる案件は断る() {
    let d = scratch("fixed-replay");
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
    let _ = std::fs::remove_dir_all(&d);
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
    let d = scratch("worlds");
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
    let python = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let lean = root().join("proofs/.lake/build/bin/rulec-recheck");
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
    let _ = std::fs::remove_dir_all(&d);
}

/// The review rule in every language that runs here. Its `final` line lists the states in an
/// order that is not the enum's, which the constants every runner prints first are held to.
#[test]
fn heldのある機械も生成したコードで回る() {
    let d = scratch("review-gen");
    let rule = write(&d, "review.rule", REVIEW);
    let out_dir = d.join("gen").to_string_lossy().into_owned();
    let (c, out) = rulec(&["gen", &rule, "--out", &out_dir]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec(&["test", &out_dir, "--format", "json"]);
    assert_eq!(c, 0, "生成したコードが食い違う:\n{out}");
    let _ = std::fs::remove_dir_all(&d);
}
