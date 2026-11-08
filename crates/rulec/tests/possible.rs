//! `possible` (§15.287): what is to happen, and the shortest calls that make it happen.
//!
//! `never`, `once` and `always` are kept by a table that does nothing. A `possible` line is the far
//! side: it holds only where calls that really happen were found, each the same case's; it is E146 only
//! where everything that can happen was looked at; and anything between is W127 under a `machine` and
//! W135 outside one. The calls are half of it, so they are held here wherever they are shown.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, ready};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec does not start");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn scratch(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("possible-{tag}"));
    let d = t.path().to_path_buf();
    (t, d)
}

fn write(dir: &Path, name: &str, src: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, src).unwrap();
    p.to_string_lossy().into_owned()
}

fn codes(src: &str) -> Vec<String> {
    let mut v: Vec<String> = rulec::check_source(src, "r.rule").iter().map(|d| d.code.to_string()).collect();
    v.sort();
    v
}

/// What a rule's `possible` lines come to, leaving out what else its check says.
fn said(src: &str) -> Vec<String> {
    codes(src).into_iter().filter(|c| matches!(c.as_str(), "E145" | "E146" | "W127" | "W135")).collect()
}

/// The findings of a rule with these lines at the end of its `machine`, in the order they are said.
fn titled(src: &str, code: &str) -> Vec<String> {
    rulec::check_source(src, "r.rule").iter().filter(|d| d.code == code).map(|d| d.title.clone()).collect()
}

/// The notes of the first finding with this code.
fn notes(src: &str, code: &str) -> String {
    rulec::check_source(src, "r.rule").into_iter().find(|d| d.code == code).map(|d| d.notes.join("\n")).unwrap_or_default()
}

/// The `possible` line of `check --format json`, each claim of it.
fn json_claims(file: &str) -> Vec<Json> {
    let (_, out) = rulec(&["check", file, "--format", "json"]);
    let line = out.lines().find(|l| l.contains("\"kind\":\"possible\"")).unwrap_or_else(|| panic!("no `possible` line:\n{out}"));
    let j = rulec::json::parse(line).unwrap();
    j.get("possible").and_then(|x| x.as_arr()).unwrap().to_vec()
}

fn field<'a>(j: &'a Json, k: &str) -> &'a str {
    j.get(k).and_then(|x| x.as_str()).unwrap_or("")
}

fn file_text(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap()
}

const ORDER: &str = "rule order(order) v1

enum state(state) = received(received) | paid(paid) | shipped(shipped) | delivered(delivered) | cancelled(cancelled)
enum step(step) = pay(pay) | ship(ship) | deliver(deliver) | cancel(cancel)

inputs
  now(now)   : state
  what(what) : step

outputs
  next_state(next_state) : state
  refund(refund)         : money[JPY]  round up(1JPY)

table moves(moves)
policy first
| now      | what    | -> next_state | -> refund |
| received | pay     | paid          | 0JPY      |
| received | cancel  | cancelled     | 0JPY      |
| paid     | ship    | shipped       | 0JPY      |
| paid     | cancel  | cancelled     | 500JPY    |
| shipped  | deliver | delivered     | 0JPY      |
| shipped  | cancel  | cancelled     | 300JPY    |
| -        | -       | now           | 0JPY      |

machine order over moves
  carry    now -> next_state
  initial  received
  final    delivered, cancelled
  never    shipped after cancelled
";

/// A case holds `amount`, and which `total` a call has moves with `extra`. The first call needs a large
/// `total` and the second a small one: every value of `amount` makes one of them, and no call the walk
/// saw makes both with one value.
const MIXED: &str = "rule t(t) v1

enum s(s) = a(a) | b(b) | c(c)

inputs
  now(now)       : s
  amount(amount) : number  range >=0 <=10
  extra(extra)   : number  range >=0 <=10

define total(total) : number = amount + extra

outputs
  nx(nx)     : s
  done(done) : bool

table m(m)
policy first
| now | total | -> nx | done  |
| a   | >=15  | b     | false |
| b   | <=2   | c     | true  |
| -   | -     | now   | false |

machine k(k) over m
  carry    now -> nx
  held     amount
  initial  a
";

const REVIEW: &str = "rule review(review) v1

enum state(state) = received(received) | reviewing(reviewing) | small_ok(small_ok) | approved(approved) | rejected(rejected)
enum event(event) = apply(apply) | amend(amend) | decide(decide) | deny(deny)

inputs
  now(now)       : state
  what(what)     : event
  amount(amount) : money[JPY]  range >=1JPY <=10000000JPY

outputs
  next_state(next_state) : state

table step(step)
policy unique
| now       | what                 | amount      | -> next_state |
| received  | apply                | <=100000JPY | small_ok      |
| received  | apply                | >100000JPY  | reviewing     |
| received  | amend, decide, deny  | -           | now           |
| reviewing | decide               | -           | approved      |
| reviewing | deny                 | -           | rejected      |
| reviewing | apply, amend         | -           | now           |
| small_ok  | amend                | >100000JPY  | reviewing     |
| small_ok  | amend                | <=100000JPY | now           |
| small_ok  | apply, decide, deny  | -           | now           |
| approved  | -                    | -           | now           |
| rejected  | -                    | -           | now           |

machine application over step
  carry    now -> next_state
  held     amount
  initial  received
  final    approved, rejected, small_ok
  possible reviewing after small_ok
";

fn with(src: &str, lines: &str) -> String {
    format!("{src}{lines}")
}

// ------------------------------------------------------------------------------------------------
// The pair: a table that does nothing keeps the guarding claims, and `possible` stops it.

#[test]
fn 何もしない表はneverを全部通し_possibleを足すと止まる() {
    let lazy = "tests/corpus/order_cancels_everything.rule";
    let (c, out) = rulec(&["check", lazy]);
    assert_eq!(c, 0, "keeps every `never` and `once` line:\n{out}");
    assert!(out.contains("warning[W125]"), "{out}");
    let (c, out) = rulec(&["check", "tests/mutants/m_e146.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("error[E146]: No sequence of calls makes `possible delivered` happen"), "{out}");
    assert!(out.contains("From received a case reaches received / cancelled. delivered is not among them."), "{out}");
    // The same order with a table that does its job.
    let (c, out) = rulec(&["check", "tests/corpus/order_happens.rule", "--show-possible"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("3. at shipped, event = deliver → delivered (table step row 7)"), "{out}");
}

#[test]
fn 日本語の版も同じように止まる() {
    let (c, _) = rulec(&["check", "tests/corpus/なんでも取り消す注文.rule"]);
    assert_eq!(c, 0);
    let (c, out) = rulec(&["check", "tests/mutants/m_e146ja.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("`possible 配達済`"), "{out}");
}

// ------------------------------------------------------------------------------------------------
// The three forms under a `machine`: holds, does not, not settled.

#[test]
fn 状態の形() {
    assert_eq!(codes(&with(ORDER, "  possible delivered\n")), Vec::<String>::new());
    assert_eq!(codes(&with(ORDER, "  possible received\n")), Vec::<String>::new(), "a case starts there");
    let src = with(ORDER, "  possible received after paid\n");
    assert_eq!(codes(&src), ["E146"]);
    assert!(notes(&src, "E146").contains("After paid a case reaches paid / shipped / cancelled / delivered."), "{}", notes(&src, "E146"));
    assert_eq!(codes(&with(MIXED, "  possible c\n")), ["W127"], "only by a sequence no one value is shown to make");
}

#[test]
fn 出力の形() {
    assert_eq!(codes(&with(ORDER, "  possible refund >0JPY after shipped\n")), Vec::<String>::new());
    let src = with(ORDER, "  possible refund >1000JPY\n");
    assert_eq!(codes(&src), ["E146"]);
    assert!(notes(&src, "E146").contains("the calls answer refund with 0JPY / 500JPY / 300JPY."), "{}", notes(&src, "E146"));
    // After the state a case ends in, no call answers it.
    assert_eq!(codes(&with(ORDER, "  possible refund >0JPY after delivered\n")), ["E146"]);
    assert_eq!(codes(&with(MIXED, "  possible done true\n")), ["W127"]);
    assert_eq!(codes(&with(MIXED, "  possible done false\n")), Vec::<String>::new());
}

/// A value the output is worked out to moves inside a cell: the far ends the walk asked answered it, or
/// the cut at the line's own boundary did.
#[test]
fn 計算で出る答えはセルの境目で切る() {
    let src = "rule r(r) v1

enum s(s) = a(a) | b(b)

inputs
  now(now)       : s
  amount(amount) : money[JPY]  range >=0JPY <=100000JPY

outputs
  nx(nx)         : s
  refund(refund) : money[JPY]  round up(1JPY)

table m(m)
policy first
| now | -> nx | refund |
| a   | b     | amount |
| b   | b     | 0JPY   |

machine k(k) over m
  carry    now -> nx
  held     amount
  initial  a
  final    b
";
    assert_eq!(said(&with(src, "  possible refund >0JPY\n")), Vec::<String>::new());
    assert_eq!(said(&with(src, "  possible refund = 70000JPY\n")), Vec::<String>::new());
    assert_eq!(said(&with(src, "  possible refund > 200000JPY\n")), ["E146"]);
}

const RETRY: &str = "tests/corpus/retry_attempts.rule";

/// The corpus's retry rule, with lines added to its machine.
fn retry(lines: &str) -> String {
    let src = file_text(RETRY);
    src.replacen("  always  attempts 3 when given_up\n", &format!("  always  attempts 3 when given_up\n{lines}"), 1)
}

#[test]
fn 持ち越す値の形() {
    assert_eq!(codes(&retry("  possible attempts 2 when pending\n")), Vec::<String>::new());
    assert_eq!(codes(&retry("  possible attempts 2\n")), Vec::<String>::new());
    let src = retry("  possible attempts 4\n");
    assert_eq!(codes(&src), ["E146"]);
    assert!(notes(&src, "E146").contains("it has attempts 0..3."), "{}", notes(&src, "E146"));
    // `when` is a claim for each state.
    let src = retry("  possible attempts 0 when pending, given_up\n");
    assert_eq!(titled(&src, "E146").len(), 1, "{:?}", titled(&src, "E146"));
    assert!(titled(&src, "E146")[0].contains("when given_up"), "{:?}", titled(&src, "E146"));
}

/// Too wide to walk one value at a time: walked in intervals, what was found again by real calls holds,
/// what the intervals rule out does not, and what only the intervals find is not settled.
#[test]
fn 持ち越す値を区間で歩く() {
    let wide = |lines: &str| {
        retry(lines).replacen("range >=0 <=3 ", "range >=0 <=100000000 ", 1)
    };
    let src = wide("  possible attempts 2 when pending\n");
    assert_eq!(codes(&src).iter().filter(|c| c.as_str() != "W127").count(), 0, "{:?}", codes(&src));
    assert!(titled(&src, "W127").iter().all(|t| !t.contains("possible")), "{:?}", titled(&src, "W127"));
    let src = wide("  possible attempts 4\n");
    assert!(codes(&src).contains(&"E146".to_string()), "{:?}", codes(&src));

    // A counter that goes up by one, to a value no walk of the real calls gets to.
    let counter = "rule counter(counter) v1

enum s(s) = open(open) | closed(closed)
enum e(e) = tick(tick) | stop(stop)

inputs
  now(now)   : s
  what(what) : e
  n(n)       : number  range >=0 <=100000000

define more(more) : number = n + 1

outputs
  nx(nx)   : s
  nn(nn)   : number  round down(1)

table m(m)
policy first
| now    | what | n        | -> nx  | nn   |
| open   | tick | <=99999999 | open | more |
| open   | stop | -        | closed | n    |
| -      | -    | -        | now    | n    |

machine k(k) over m
  carry    now -> nx
  carry    n -> nn
  initial  open
  initial  n 0
  final    closed
  possible n = 70000000
";
    let found = titled(counter, "W127");
    assert!(found.iter().any(|t| t.contains("possible n = 70000000")), "over the intervals, and not from real calls: {:?}", codes(counter));
    assert!(!codes(counter).contains(&"E146".to_string()), "{:?}", codes(counter));
}

// ------------------------------------------------------------------------------------------------
// One case's calls.

#[test]
fn heldの世界をまたがない() {
    // Held, no value of the amount makes both calls.
    assert_eq!(titled(REVIEW, "E146").len(), 1, "{:?}", codes(REVIEW));
    // Free to change, the same table shows a sequence.
    let free = REVIEW.replace("  held     amount\n", "");
    assert!(titled(&free, "E146").is_empty(), "{:?}", codes(&free));
}

#[test]
fn 途中で値を変えないと通れない並びを成り立つと数えない() {
    let src = with(MIXED, "  possible c\n");
    assert_eq!(codes(&src), ["W127"]);
    assert!(notes(&src, "W127").contains("needs amount to change partway"), "{}", notes(&src, "W127"));
    // The call that gets to b is one value's, and it is shown.
    assert_eq!(codes(&with(MIXED, "  possible b\n")), Vec::<String>::new());
}

// ------------------------------------------------------------------------------------------------
// A list is a claim for each.

#[test]
fn afterの組を一つずつ分ける() {
    let src = with(ORDER, "  possible cancelled, received after paid, shipped\n");
    let mut said: Vec<String> = titled(&src, "E146");
    said.sort();
    assert_eq!(
        said,
        [
            "No sequence of calls makes `possible received after paid` happen",
            "No sequence of calls makes `possible received after shipped` happen",
        ]
    );
    let (_t, d) = scratch("pairs");
    let f = write(&d, "o.rule", &src);
    let claims = json_claims(&f);
    let got: Vec<(&str, &str)> = claims.iter().map(|c| (field(c, "claim"), field(c, "result"))).collect();
    assert_eq!(
        got,
        [
            ("possible cancelled after paid", "holds"),
            ("possible cancelled after shipped", "holds"),
            ("possible received after paid", "absent"),
            ("possible received after shipped", "absent"),
        ]
    );
}

// ------------------------------------------------------------------------------------------------
// Outside a `machine`.

const QUOTE: &str = "tests/corpus/repair_quote.rule";

fn quote(line: &str) -> String {
    format!("{}{line}\n", file_text(QUOTE))
}

#[test]
fn 一番上の階層は一度の呼び出し() {
    assert_eq!(codes(&file_text(QUOTE)), Vec::<String>::new());
    let src = quote("possible tier = basic, approved = true");
    assert_eq!(codes(&src), ["E146"]);
    assert!(notes(&src, "E146").contains("approved answers false"), "{}", notes(&src, "E146"));
    // What the door refuses is no call.
    let src = quote("possible tier = gold, tools = 0");
    assert_eq!(codes(&src), ["E146"]);
    assert!(notes(&src, "E146").contains("No call passes these inputs"), "{}", notes(&src, "E146"));
    let src = quote("possible fee > 1000USD");
    assert_eq!(codes(&src), ["E146"]);
    assert!(notes(&src, "E146").contains("fee answers 10USD / 150USD / 200USD / 50USD"), "{}", notes(&src, "E146"));
}

#[test]
fn 条件の書き方はneverと同じ() {
    for line in ["possible tier = gold | basic, approved = true", "possible tier = not: basic, fee = 50USD", "possible tools >= 6 <= 7, approved = true"] {
        assert_eq!(codes(&quote(line)), Vec::<String>::new(), "{line}");
    }
}

#[test]
fn 機械のある規則でも一番上の階層は一度の呼び出し() {
    let src = format!("{ORDER}\npossible what = cancel, refund = 500JPY\n");
    assert_eq!(codes(&src), Vec::<String>::new());
    // In one call: `shipped` here is a value of the input, and no call that passes it pays 500JPY.
    let src = format!("{ORDER}\npossible now = shipped, refund = 500JPY\n");
    assert_eq!(codes(&src), ["E146"]);
}

/// A rule whose `total` is `raw_total`, worked out from two inputs as `expr`, with these lines.
fn computed(expr: &str, lines: &str) -> String {
    format!(
        "rule t(t) v1

inputs
  qty(qty)     : number  range >=1 <=1000000
  price(price) : number  range >=1 <=1000000

define raw_total(raw_total) : number = {expr}

outputs
  total(total) : number  round down(1)

table totals(totals)
policy first
| -> total  |
| raw_total |

{lines}"
    )
}

/// A number the rule computes from two inputs, on a value only some calls have: neither a line nor
/// the divisors settle it, and it is not said to hold. Beside it, a value past what the inputs reach
/// is still said not to happen.
#[test]
fn w135_決めきれないことを成り立つと言わない() {
    let src = computed("qty * (price + 50)", "possible total = 80779853376\npossible total = 5000000000000\n");
    assert_eq!(said(&src), ["E146", "W135"]);
    let open = titled(&src, "W135");
    assert!(open.iter().any(|t| t.contains("total = 80779853376")), "{open:?}");
}

/// A product on one value is a matter of its divisors (§15.291). 80,779,853,376 is 123,456 × 654,321,
/// which no move of one input along a line lands on; 1,000,003 is a prime past both ranges, which no
/// line rules out; 1,000,001 is 101 × 9,901, inside both.
#[test]
fn 積の値は約数で決まる() {
    let src = computed("qty * price", "possible total = 80779853376\n");
    assert_eq!(said(&src), Vec::<String>::new());
    let (_t, d) = scratch("product");
    let f = write(&d, "p.rule", &src);
    let claims = json_claims(&f);
    let inputs = claims[0].get("witness").and_then(|w| w.get("inputs")).unwrap();
    let of = |k: &str| inputs.get(k).and_then(|x| x.as_int()).unwrap_or_else(|| panic!("{k}: {inputs:?}"));
    assert_eq!(of("qty") * of("price"), 80779853376);
    assert_eq!(said(&computed("qty * price", "possible total = 1000003\n")), ["E146"]);
    assert_eq!(said(&computed("qty * price", "possible total = 1000001\n")), Vec::<String>::new());
    // A factor and an addend that are plain numbers: 3 × 1,000,003 + 1 is the prime again, and
    // 3,000,005 is no whole product at all.
    assert_eq!(said(&computed("qty * price * 3 + 1", "possible total = 242339560129\n")), Vec::<String>::new());
    assert_eq!(said(&computed("qty * price * 3 + 1", "possible total = 3000010\n")), ["E146"]);
    assert_eq!(said(&computed("qty * price * 3 + 1", "possible total = 3000005\n")), ["E146"]);
    // An input multiplied in twice: 999,999 squared, and one past it.
    assert_eq!(said(&computed("qty * qty", "possible total = 999998000001\n")), Vec::<String>::new());
    assert_eq!(said(&computed("qty * qty", "possible total = 999998000002\n")), ["E146"]);
}

/// A cell no input was built for leaves a line open only where its answer could meet the line
/// (§15.291). `possible total > 1000000000` cuts `raw_total` at 1,000,000,000, which `qty × qty + qty`
/// never lands on and the walk builds nothing for. `total` there is 1,000,000,000 whatever the call,
/// so the cell says nothing about `total = 0` or `total = 1`, which no call reaches.
#[test]
fn 答えが届かない区画は行を決めきれなくしない() {
    let src = computed("qty * qty + qty", "possible total = 1\npossible total > 1000000000\n");
    assert_eq!(said(&src), ["E146"]);
    // Where the answer could meet the line, the cell still leaves it open.
    let src = computed("qty * qty + qty", "possible total = 1000000000\n");
    assert_eq!(said(&src), ["W135"]);
    // A value past both ranges' product, beside a line whose cut leaves cells of its own.
    assert_eq!(said(&computed("qty * price", "possible total = 999999999989\npossible total > 1000000000\n")), ["E146"]);
}

// ------------------------------------------------------------------------------------------------
// What is written wrong.

#[test]
fn e145_書き方の誤り() {
    for (line, why) in [
        ("  possible arrived\n", "not a state"),
        ("  possible delivered after nowhere\n", "not a state"),
        ("  possible unknown >0\n", "knows no such name"),
        ("  possible refund >0JPY when paid\n", "`when` on an output"),
        ("  possible refund 5\n", "a cell its type cannot hold"),
        ("  possible delivered, refund >0JPY\n", "the two forms mixed"),
        ("  possible delivered after paid when paid\n", "`after` and `when`"),
    ] {
        assert!(codes(&with(ORDER, line)).contains(&"E145".to_string()), "{why}: {line} {:?}", codes(&with(ORDER, line)));
    }
    for line in ["possible tool = 0", "possible tier = silver", "possible tier = gold, tier = basic", "possible fee = 5"] {
        assert!(codes(&quote(line)).contains(&"E145".to_string()), "{line}: {:?}", codes(&quote(line)));
    }
    // A carried value with `after`, and an output with `when`.
    assert!(codes(&retry("  possible attempts 2 after pending\n")).contains(&"E145".to_string()));
}

/// `possible` is read by where it stands: it stays a name.
#[test]
fn possibleは名前に使える() {
    let src = "rule t(t) v1

inputs
  possible(possible) : bool

outputs
  ok(ok) : bool

table x(x)
policy first
| possible | -> ok |
| true     | true  |
| false    | false |

possible possible = true, ok = true
";
    assert_eq!(said(src), Vec::<String>::new());
    assert!(!rulec::has_error(&rulec::check_source(src, "r.rule")));
}

// ------------------------------------------------------------------------------------------------
// Showing it.

#[test]
fn show_possibleの文字() {
    let (c, out) = rulec(&["check", "tests/corpus/order_happens.rule", "--show-possible"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("order_happens.rule:36 possible delivered\n  Calls (from received):\n    1. at received, event = pay → paid (table step row 1)\n"), "{out}");
    assert!(out.contains("(refund = 300JPY, table step row 8)"), "{out}");
    assert!(out.contains("possible event = cancel, refund = 500JPY\n  state = paid, event = cancel → next_state = cancelled, refund = 500JPY"), "{out}");
    let (_, plain) = rulec(&["check", "tests/corpus/order_happens.rule"]);
    assert!(!plain.contains("Calls (from"), "only when asked:\n{plain}");
    // One call, and where a case starts.
    let (_t, d) = scratch("start");
    let f = write(&d, "o.rule", &with(ORDER, "  possible received\n"));
    let (_, out) = rulec(&["check", &f, "--show-possible"]);
    assert!(out.contains("A case starts here (received), with no call needed."), "{out}");
}

#[test]
fn jsonの配列() {
    let claims = json_claims("tests/corpus/order_happens.rule");
    let first = &claims[0];
    assert_eq!(field(first, "claim"), "possible delivered");
    assert_eq!(field(first, "result"), "holds");
    assert_eq!(first.get("line").and_then(|x| x.as_int()), Some(36.into()));
    let trace = first.get("witness").and_then(|w| w.get("trace")).and_then(|t| t.as_arr()).unwrap();
    assert_eq!(trace.len(), 3);
    let step = &trace[0];
    assert_eq!(step.get("inputs").and_then(|i| i.get("event")).and_then(|x| x.as_str()), Some("pay"));
    assert_eq!(step.get("outputs").and_then(|i| i.get("next_state")).and_then(|x| x.as_str()), Some("paid"));
    assert_eq!(step.get("rows").and_then(|r| r.as_arr()).map(|r| r.len()), Some(1));
    // One call: its inputs and answer, with no trace.
    let one = claims.last().unwrap();
    assert_eq!(field(one, "claim"), "possible event = cancel, refund = 500JPY");
    let w = one.get("witness").unwrap();
    assert!(w.get("trace").is_none());
    assert_eq!(w.get("outputs").and_then(|o| o.get("refund")).and_then(|x| x.as_int()), Some(500));
    // No `possible` line, no change to the output.
    let (_, out) = rulec(&["check", "tests/corpus/order_lifecycle.rule", "--format", "json"]);
    assert!(!out.contains("\"kind\":\"possible\""), "{out}");
    // What is not shown says why.
    let absent = json_claims("tests/mutants/m_e146_call.rule");
    let last = absent.last().unwrap();
    assert_eq!(field(last, "result"), "absent");
    assert!(field(last, "why").contains("approved answers false"), "{}", field(last, "why"));
}

#[test]
fn docに起きることが載る() {
    let (c, md) = rulec(&["doc", "tests/corpus/order_happens.rule"]);
    assert_eq!(c, 0, "{md}");
    assert!(md.contains("### What happens"), "{md}");
    assert!(md.contains("**`possible refund >0JPY after shipped`**"), "{md}");
    assert!(md.contains("| 3 | shipped | cancel | cancelled | 300JPY | table step row 8 |"), "{md}");
    assert!(md.contains("## What happens"), "the line outside the machine: {md}");
    assert!(md.contains("`possible event = cancel, refund = 500JPY`: `state = paid, event = cancel"), "{md}");
    let (c, md) = rulec(&["doc", QUOTE]);
    assert_eq!(c, 0);
    let (nv, ps) = (md.find("## These combinations do not happen").unwrap(), md.find("## What happens").unwrap());
    assert!(nv < ps, "beside the `never` lines");
    let (c, page) = rulec(&["doc", "tests/corpus/order_happens.rule", "--format", "html"]);
    assert_eq!(c, 0, "{page}");
    assert!(page.contains("What happens"), "HTML");
    assert!(page.contains("possible delivered"), "HTML");
}

#[test]
fn apiが行を言う() {
    let (c, out) = rulec(&["api", "tests/corpus/order_happens.rule"]);
    assert_eq!(c, 0, "{out}");
    let j = rulec::json::parse(out.trim()).unwrap();
    let m = j.get("machine").and_then(|x| x.get("possible")).and_then(|x| x.as_arr()).unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(field(&m[0], "said"), "possible delivered");
    assert_eq!(m[0].get("states").and_then(|x| x.as_arr()).map(|x| x.len()), Some(1));
    assert_eq!(field(&m[2], "output"), "refund");
    assert_eq!(field(&m[2], "cell"), ">0JPY");
    let top = j.get("possible").and_then(|x| x.as_arr()).unwrap();
    assert_eq!(field(&top[0], "said"), "possible event = cancel, refund = 500JPY");
    // Only where there is one.
    let (_, out) = rulec(&["api", "tests/corpus/order_lifecycle.rule"]);
    assert!(!out.contains("\"possible\""), "{out}");
    let (_, out) = rulec(&["api", "tests/corpus/retry_attempts.rule"]);
    assert!(!out.contains("\"possible\""), "{out}");
}

#[test]
fn fmtは揃えを変えない() {
    // A block with the longest word lines up to it, and a block without it stays as it was.
    let (_t, d) = scratch("fmt");
    let f = write(&d, "o.rule", &with(ORDER.replace("  carry    ", "  carry ").replace("  initial  ", "  initial ").replace("  final    ", "  final ").replace("  never    ", "  never ").as_str(), "  possible delivered\n"));
    let (c, out) = rulec(&["fmt", &f]);
    assert_eq!(c, 0, "{out}");
    let back = std::fs::read_to_string(&f).unwrap();
    assert!(back.contains("  carry    now -> next_state\n  initial  received\n"), "{back}");
    assert!(back.contains("  possible delivered\n"), "{back}");
    let (c, out) = rulec(&["fmt", "--check", RETRY]);
    assert_eq!(c, 0, "a file with no `possible` line is as it was: {out}");
    let (c, out) = rulec(&["fmt", "--check", "tests/corpus/order_happens.rule"]);
    assert_eq!(c, 0, "{out}");
}

// ------------------------------------------------------------------------------------------------
// The calls in the suite: every language's runner plays them.

/// The lines `vectors --out` writes for a machine's sequences of calls.
fn trace_lines(rule: &str, tag: &str) -> Vec<Json> {
    let (_t, d) = scratch(tag);
    let out = d.join("v").to_string_lossy().into_owned();
    let (c, said) = rulec(&["vectors", rule, "--out", &out]);
    assert_eq!(c, 0, "{said}");
    let p = std::fs::read_dir(&out).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.to_string_lossy().ends_with(".traces.jsonl")).unwrap_or_else(|| panic!("no traces under {out}"));
    std::fs::read_to_string(p).unwrap().lines().map(|l| rulec::json::parse(l).unwrap()).collect()
}

/// The calls of the trace made for this reason.
fn steps_of<'a>(lines: &'a [Json], why: &str) -> Vec<&'a Json> {
    let at = lines.iter().position(|l| field(l, "why") == why).unwrap_or_else(|| panic!("no trace for `{why}`"));
    let mut out = vec![&lines[at]];
    out.extend(lines[at + 1..].iter().take_while(|l| field(l, "step") == "next"));
    out
}

#[test]
fn 並びがスイートに入る() {
    let lines = trace_lines("tests/corpus/order_happens.rule", "traces");
    let steps = steps_of(&lines, "possible refund >0JPY after shipped");
    assert_eq!(steps.len(), 3);
    assert_eq!(field(steps[0], "state"), "received");
    assert_eq!(steps[2].get("in").and_then(|i| i.get("event")).and_then(|x| x.as_str()), Some("cancel"));
    // Over a value carried along as well.
    let (_d0, d) = scratch("traces-more");
    let f = write(&d, "retry.rule", &retry("  possible given_up after pending\n"));
    let lines = trace_lines(&f, "traces-more2");
    assert_eq!(steps_of(&lines, "possible given_up after pending").len(), 3);
}

#[test]
fn 一度の呼び出しは普通のベクトル() {
    let (c, out) = rulec(&["vectors", QUOTE]);
    assert_eq!(c, 0, "{out}");
    // The call that shows `possible fee > 100USD, tier = basic`: it stays in the suite, whatever the
    // audit would keep on its own.
    assert!(out.lines().any(|l| l.contains("\"tier\":\"basic\"") && l.contains("\"tools\":9") && l.contains("\"fee\":150")), "{out}");
    assert!(out.lines().any(|l| l.contains("\"tier\":\"gold\"") && l.contains("\"approved\":true")), "{out}");
}

#[test]
fn 生成したコードが並びを再生する() {
    if !need(Need::Python) {
        return;
    }
    let (_t, d) = scratch("replay");
    let out_dir = d.join("gen").to_string_lossy().into_owned();
    let (c, out) = rulec(&rulec::targets::up_to_0_40(&["gen", "tests/corpus/order_happens.rule", "--out", &out_dir]));
    assert_eq!(c, 0, "{out}");
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "en")
        .env("TMPDIR", tmpdir_in(Path::new(&out_dir)))
        .current_dir(root())
        .args(["test", &out_dir, "--format", "json"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert_eq!(o.status.code(), Some(0), "the generated code plays the calls of the `possible` lines:\n{said}");
}

// ------------------------------------------------------------------------------------------------
// The certificate: a path over the recorded transitions, from the initial state, ending where the line
// says. Both re-checkers run it.

fn certificate(rule: &str) -> String {
    let (c, out) = rulec(&["certificate", rule]);
    assert_eq!(c, 0, "{out}");
    out
}

fn feed(cmd: &mut Command, input: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = cmd.current_dir(root()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("does not start");
    p.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

fn lean_bin() -> Option<PathBuf> {
    let p = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    ready(Need::Lean, || p.exists(), "ritsu の proofs/ が build されていない（そこで lake build で作る）").then_some(p)
}

/// The rule's certificate passes both, and each forgery of it fails both.
fn holds_and_forged(rule: &str, forged: &[(&str, &str)]) {
    let cert = certificate(rule);
    let python = need(Need::Python);
    let lean = lean_bin();
    if python {
        let (c, said) = feed(Command::new("python3").args(["tools/recheck.py", "--rule", rule]), &cert);
        assert_eq!(c, 0, "{said}");
    }
    if let Some(bin) = &lean {
        let (c, said) = feed(Command::new(bin).args(["--rule", rule]), &cert);
        assert_eq!(c, 0, "{said}");
    }
    for (from, to) in forged {
        assert!(cert.contains(from), "the certificate has no `{from}`:\n{cert}");
        let bad = cert.replacen(from, to, 1);
        if python {
            let (c, said) = feed(Command::new("python3").args(["tools/recheck.py"]), &bad);
            assert_ne!(c, 0, "forged {from} -> {to}, and Python passes:\n{said}");
        }
        if let Some(bin) = &lean {
            let (c, said) = feed(&mut Command::new(bin), &bad);
            assert_ne!(c, 0, "forged {from} -> {to}, and Lean passes:\n{said}");
        }
    }
}

#[test]
fn 証明書に並びが載り_両方で確かめる() {
    let cert = rulec::json::parse(certificate("tests/corpus/order_happens.rule").trim()).unwrap();
    let m = cert.get("machine").unwrap();
    let ps = m.get("possible").and_then(|x| x.as_arr()).unwrap();
    assert_eq!(ps.len(), 4, "one per pair");
    assert_eq!(field(&ps[0], "form"), "state");
    assert_eq!(ps[0].get("path").and_then(|x| x.as_arr()).map(|x| x.len()), Some(3));
    assert_eq!(field(&ps[3], "form"), "output");
    assert_eq!(m.get("uncertified").and_then(|x| x.as_arr()).map(|x| x.len()), Some(0));
    holds_and_forged(
        "tests/corpus/order_happens.rule",
        // The state it is said to end in, and where the line's first call is said to start.
        &[("\"form\":\"state\",\"state\":3", "\"form\":\"state\",\"state\":1"), ("\"path\":[{\"state\":0,", "\"path\":[{\"state\":1,")],
    );
}

#[test]
fn 持ち越す値の並びも両方で確かめる() {
    let (_t, d) = scratch("cert-more");
    let f = write(&d, "retry.rule", &retry("  possible given_up after pending\n  possible attempts 2 when pending\n  possible attempts 2 when collected\n"));
    let cert = rulec::json::parse(certificate(&f).trim()).unwrap();
    let ps = cert.get("machine").and_then(|m| m.get("carried")).and_then(|c| c.get("possible")).and_then(|x| x.as_arr()).unwrap();
    assert_eq!(ps.len(), 3);
    assert_eq!(field(&ps[1], "form"), "values");
    holds_and_forged(&f, &[("\"form\":\"state\",\"state\":2", "\"form\":\"state\",\"state\":1"), ("\"when\":0", "\"when\":1")]);
}

/// Each line outside the machine lists its call, and neither program claims to have worked it out.
#[test]
fn 一番上の階層の呼び出しは載せて_言ったことにする() {
    let text = certificate(QUOTE);
    let cert = rulec::json::parse(text.trim()).unwrap();
    let top = cert.get("possible").and_then(|x| x.as_arr()).unwrap();
    assert_eq!(top.len(), 3);
    assert_eq!(top[2].get("witness").and_then(|w| w.get("inputs")).and_then(|i| i.get("tools")).and_then(|x| x.as_int()), Some(9));
    if need(Need::Python) {
        let (c, said) = feed(Command::new("python3").args(["tools/recheck.py", "--rule", QUOTE]), &text);
        assert_eq!(c, 0, "{said}");
        assert!(said.contains("`possible approved = true`: the call that shows it is listed"), "{said}");
    }
    if let Some(bin) = lean_bin() {
        let (c, said) = feed(Command::new(bin).args(["--rule", QUOTE]), &text);
        assert_eq!(c, 0, "{said}");
        assert!(said.contains("`possible approved = true`: the call that shows it is listed"), "{said}");
    }
}
