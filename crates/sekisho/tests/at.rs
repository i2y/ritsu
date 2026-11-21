//! How far a gate allows a workflow an action at one call of its flow (`Gates::allowed_at`, DESIGN
//! 2.12, 4.6, 8.3): over the combinations the runs of the flow that reach the call can come to.
//! dandori says what each call gives and what the runs hold there, as `ritsu check` asks it; the
//! gate is asked with each call. The project is a claims desk (fictional): a workflow pays a
//! claim through an operation the gate guards.

mod common;

use ritsu_ports::{AllowedAt, Allowance, Found, Gates};
use sekisho::ports::Engine;

const API: &str = r#"{
  "openapi": "3.1.0",
  "info": { "title": "Claims desk", "version": "1.0.0", "description": "Pay a claim" },
  "servers": [ { "url": "https://desk.example" } ],
  "paths": {
    "/claims/{claimId}/payments": {
      "post": {
        "operationId": "payClaim",
        "summary": "Pay a claim",
        "parameters": [ { "name": "claimId", "in": "path", "required": true, "schema": { "type": "string" } } ],
        "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["amount", "channel"], "properties": {
          "amount": { "type": "integer", "minimum": 1, "maximum": 10000 },
          "channel": { "type": "string", "enum": ["bank", "cash"] } } } } } },
        "responses": { "200": { "description": "Paid", "content": { "application/json": { "schema": { "type": "object", "required": ["paid"], "properties": { "paid": { "type": "boolean" } } } } } }, "403": { "description": "Denied" } }
      }
    }
  }
}
"#;

/// Who pays a claim: the desk up to 500 yen, a person above it.
const WHO: &str = "rule who_pays(k) v1
description \"Who pays a claim: the desk up to 500 yen, a person above it\"

enum payer = desk \"Desk\" | person \"Person\"

inputs
  amount : money[JPY]  range >=1JPY <=10_000JPY

outputs
  who : payer

table t
| amount   | -> who |
| <=500JPY | desk   |
| >500JPY  | person |

examples
| amount | -> who |
| 500JPY | desk   |
| 501JPY | person |
";

/// The size of a claim: small up to 300 yen, large above it.
const SIZE: &str = "rule claim_size(size_of) v1
description \"The size of a claim: small up to 300 yen, large above it\"

enum size = small \"Small\" | large \"Large\"

inputs
  amount : money[JPY]  range >=1JPY <=10_000JPY

outputs
  size : size

table t
| amount   | -> size |
| <=300JPY | small   |
| >300JPY  | large   |

examples
| amount | -> size |
| 300JPY | small   |
| 301JPY | large   |
";

/// Who pays a claim by its amount and the days it waited: the desk up to 500 yen within a week, a
/// person otherwise.
const LATE: &str = "rule who_pays_late(late) v1
description \"Who pays a claim: the desk up to 500 yen within a week, a person otherwise\"

enum payer = desk \"Desk\" | person \"Person\"

inputs
  amount : money[JPY]  range >=1JPY <=10_000JPY
  days   : number  range >=0 <=30

outputs
  who : payer

table t
| amount   | days | -> who |
| <=500JPY | <=7  | desk   |
| <=500JPY | >7   | person |
| >500JPY  | -    | person |

examples
| amount | days | -> who |
| 500JPY | 7    | desk   |
| 500JPY | 8    | person |
| 501JPY | 0    | person |
";

/// The flow: `inputs` are its inputs after `claim_id`, `body` its statements.
fn flow(inputs: &str, body: &str) -> String {
    format!(
        "workflow pay v1
description \"Pay the claims the desk may pay\"

use rule k from \"../rules/who.rule\"
use openapi desk from \"../api/desk.json\"

enum channel = bank | cash
enum kind = routine | special

record Paid
  paid : bool

record Claim
  id    : string
  total : money[JPY]  range >=1JPY <=10_000JPY

record Quote
  total : money[JPY]  range >=1JPY <=10_000JPY
  days  : int  range >=0 <=30

inputs
  claim_id : string
{inputs}
outputs
  done : bool

task pay_claim(claimId: string, amount: money[JPY] range >=1JPY <=10_000JPY, channel: channel) -> Paid
  http POST desk \"/claims/{{claimId}}/payments\"
  idempotent

task quote(claimId: string) -> Quote
  lambda \"arn:aws:lambda:us-east-1:123456789012:function:quote\"
  idempotent

flow
{body}"
    )
}

/// The inputs most flows take.
const INPUTS: &str = "  amount   : money[JPY]  range >=1JPY <=10_000JPY\n  channel  : channel\n";

/// The gate: `uses` its `use rule` lines, `holds` the lines under its workflow, `attributes` the
/// attributes of a claim, `context` the values the action computes, `conds` the conditions of the
/// workflow's permit.
fn gate(uses: &str, holds: &str, attributes: &str, context: &str, conds: &str) -> String {
    let context = if context.is_empty() { String::new() } else { format!("  context\n{context}") };
    format!(
        "gate desk v1
description \"Who pays a claim\"
namespace Desk

{uses}use openapi desk from \"../api/desk.json\"

enum channel = bank | cash
enum kind = routine | special

workflow pay from \"../flows/pay.flow\"
  description \"Pays the claims the desk may pay\"
{holds}
resource Claim
  description \"A claim\"
  attributes
{attributes}
action pay_claim
  description \"Pay a claim\"
  guards desk payClaim
  principal Workflow
  resource Claim from claimId
  input
    amount  : money[JPY]  range >=1JPY <=10_000JPY
    channel : channel
{context}
permit the_workflow_pays
  description \"The workflow pays the claims the desk may pay\"
  principal is workflow pay
  action pay_claim
{conds}"
    )
}

const AMOUNT: &str = "    amount : money[JPY]  range >=1JPY <=10_000JPY\n";
const HOLDS_AMOUNT: &str = "  resource Claim from claim_id\n    amount = amount\n";

/// The API, the rules, the flow and the gate, in `t`; the root they are in.
fn laid_out(t: &ritsu_testkit::TempDir, flow: &str, gate: &str) -> std::path::PathBuf {
    for (file, text) in [("api/desk.json", API), ("rules/who.rule", WHO), ("rules/size.rule", SIZE), ("rules/late.rule", LATE), ("flows/pay.flow", flow), ("gates/desk.gate", gate)] {
        let path = t.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::canonicalize(t.path()).unwrap()
}

/// The codes `check` gives the gate, as `ritsu check` checks it with every language joined, and
/// what it prints in English and in Japanese, the directory left out.
fn checked(flow: &str, gate: &str) -> (Vec<&'static str>, String) {
    let t = ritsu_testkit::TempDir::new("at-check");
    let root = laid_out(&t, flow, gate);
    let path = root.join("gates/desk.gate");
    let o = sekisho::check::check_file(path.to_str().unwrap(), &common::joined(), &sekisho::check::Options { root: Some(root.clone()), ..sekisho::check::Options::default() }).unwrap();
    let shown = format!("{}/", root.display());
    let text: String = [ritsu_base::text::Lang::En, ritsu_base::text::Lang::Ja].iter().flat_map(|l| o.diags.iter().map(move |d| d.render(*l))).collect();
    (o.diags.iter().map(|d| d.code).collect(), text.replace(&shown, ""))
}

/// The answer at each call of `pay_claim` in the flow, in the order of the lines.
fn asked(flow: &str, gate: &str) -> Vec<AllowedAt> {
    let t = ritsu_testkit::TempDir::new("at");
    let root = laid_out(&t, flow, gate);
    let p = common::ports();
    let ports = ritsu_ports::Ports { rules: p.rules.clone(), dates: p.dates.clone(), books: p.books.clone() };
    let (_, calls) = p.flows.operation_calls(&root, "flows/pay.flow", &ports).unwrap_or_else(|s| panic!("the flow: {s:?}\n{flow}"));
    let e = Engine::new(common::joined());
    calls.iter().filter(|c| c.task == "pay_claim").map(|c| e.allowed_at(&root, "gates/desk.gate", "pay_claim", "pay", c).unwrap_or_else(|s| panic!("the gate: {s:?}\n{gate}"))).collect()
}

fn word(a: &AllowedAt) -> &'static str {
    match &a.allowance {
        Found::Value(Allowance::Always) => "always",
        Found::Value(Allowance::Sometimes { .. }) => "sometimes",
        Found::Value(Allowance::Never) => "never",
        Found::Undecided(_) => "undecided",
    }
}

fn words(a: &[AllowedAt]) -> Vec<&'static str> {
    a.iter().map(word).collect()
}

/// The line of a file that holds `piece`, from 1.
fn line_of(text: &str, piece: &str) -> usize {
    text.lines().position(|l| l.contains(piece)).unwrap_or_else(|| panic!("no line holds {piece:?}")) + 1
}

fn en(texts: &[ritsu_base::text::Text]) -> Vec<&str> {
    texts.iter().map(|t| t.en.as_str()).collect()
}

/// The desk's arm and the person's, each paying the claim.
const BOTH_ARMS: &str = "  let r = k(amount: amount)
  match r.who
    desk =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = true
    person =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = false
";

/// The gate computes who pays from the claim's amount, as the flow does from its own: under the
/// arm of an answer, the call is asked with the answer.
#[test]
fn a_call_in_an_arm_of_a_rules_answer() {
    let uses = "use rule k from \"../rules/who.rule\"\n";
    let context = "    who = k(amount: resource.amount).who\n";
    let g = gate(uses, HOLDS_AMOUNT, AMOUNT, context, "  when who is desk\n");
    let a = asked(&flow(INPUTS, BOTH_ARMS), &g);
    assert_eq!(words(&a), ["always", "never"]);
    // each answer rests on the line that says the claim's amount is the flow's, and says what
    // the arm holds in the gate's words
    for (x, value) in a.iter().zip(["desk", "person"]) {
        assert_eq!(x.rests_on.iter().map(|(t, l)| (t.en.as_str(), *l)).collect::<Vec<_>>(), [("`amount = amount` under `resource Claim from claim_id`", line_of(&g, "    amount = amount"))]);
        assert_eq!(en(&x.held), [format!("`k(amount: resource.amount).who` (the flow's `r.who`) is `{value}`")]);
        assert!(x.unheld.is_empty() && x.unrelated.is_empty(), "{x:?}");
        assert!(x.held.iter().all(|t| !t.ja.is_empty()));
    }
    // the gate says nothing of what the flow holds of the claim: every combination of the gate,
    // with what the answer reads that nothing is said of (the flow's amount is the input of the
    // action the call gives it to, which the answer does not read)
    let a = asked(&flow(INPUTS, BOTH_ARMS), &gate(uses, "", AMOUNT, context, "  when who is desk\n"));
    assert_eq!(words(&a), ["sometimes", "sometimes"]);
    for x in &a {
        assert_eq!(x.unrelated, ["resource.amount"]);
        assert!(x.rests_on.is_empty() && x.held.is_empty() && x.unheld.is_empty(), "{x:?}");
    }
    // a `resource` line with nothing under it says the flow holds nothing of the claim: every
    // value of its attributes, and nothing left to say
    let a = asked(&flow(INPUTS, BOTH_ARMS), &gate(uses, "  resource Claim from claim_id\n", AMOUNT, context, "  when who is desk\n"));
    assert_eq!(words(&a), ["sometimes", "sometimes"]);
    assert!(a.iter().all(|x| x.unrelated.is_empty() && x.rests_on.is_empty()), "{a:?}");
    // after the `match`, the runs of both arms come to the call: nothing is held
    let after = "  let r = k(amount: amount)\n  match r.who\n    desk => pass\n    person => pass\n  pay_claim(claimId: claim_id, amount: amount, channel: channel)\n  succeed done = true\n";
    let a = asked(&flow(INPUTS, after), &g);
    assert_eq!(words(&a), ["sometimes"]);
    assert!(a[0].held.is_empty() && a[0].unheld.is_empty() && a[0].unrelated.is_empty(), "{:?}", a[0]);
    let Found::Value(Allowance::Sometimes { allowed, denied }) = &a[0].allowance else { unreachable!() };
    assert!(allowed.en.contains("who: desk") && denied.en.contains("who: person"), "{allowed:?} {denied:?}");
}

/// The flow's rule is none the gate uses: its answer is counted beside what the gate reads of the
/// same attribute, by the amounts each answer comes from.
#[test]
fn a_rule_of_the_flow_the_gate_does_not_use() {
    // the gate compares the amount itself
    let g = gate("", HOLDS_AMOUNT, AMOUNT, "", "  when resource.amount <= 500JPY\n");
    let a = asked(&flow(INPUTS, BOTH_ARMS), &g);
    assert_eq!(words(&a), ["always", "never"]);
    assert_eq!(en(&a[0].held), ["`who_pays(amount: resource.amount).who` (the flow's `r.who`) is `desk`"]);
    // the gate reads another rule of the same amount, which cuts at 300 yen: the desk's arm (up
    // to 500) is small or large, the person's (above 500) is large
    let uses = "use rule size_of from \"../rules/size.rule\"\n";
    let context = "    size = size_of(amount: resource.amount).size\n";
    let small = asked(&flow(INPUTS, BOTH_ARMS), &gate(uses, HOLDS_AMOUNT, AMOUNT, context, "  when size is small\n"));
    assert_eq!(words(&small), ["sometimes", "never"]);
    let Found::Value(Allowance::Sometimes { allowed, denied }) = &small[0].allowance else { unreachable!() };
    assert!(allowed.en.contains("size: small") && denied.en.contains("size: large"), "{allowed:?} {denied:?}");
    let large = asked(&flow(INPUTS, BOTH_ARMS), &gate(uses, HOLDS_AMOUNT, AMOUNT, context, "  when size is large\n"));
    assert_eq!(words(&large), ["sometimes", "always"]);
}

/// An input of the action is what the call gives the operation's argument of its name: no line of
/// the gate says so, and no answer rests on one.
#[test]
fn what_the_call_gives_the_inputs_of_the_action() {
    let g = gate("", "", AMOUNT, "", "  when amount <= 500JPY\n");
    let pay = "  pay_claim(claimId: claim_id, amount: amount, channel: channel)\n  succeed done = true\n";
    let ranged = |range: &str| format!("  amount   : money[JPY]  range {range}\n  channel  : channel\n");
    // every amount the flow can give is one the permit lets in, or none is
    let a = asked(&flow(&ranged(">=1JPY <=500JPY"), pay), &g);
    assert_eq!(words(&a), ["always"]);
    assert!(a[0].rests_on.is_empty() && a[0].unrelated.is_empty() && a[0].unheld.is_empty(), "{:?}", a[0]);
    assert_eq!(en(&a[0].held), ["`amount` (what the call gives, `amount`) is 1JPY to 500JPY"]);
    assert_eq!(a[0].held[0].ja, "`amount`（呼び出しが渡す `amount`）は 1JPY〜500JPY");
    let a = asked(&flow(&ranged(">=501JPY <=1_000JPY"), pay), &g);
    assert_eq!(words(&a), ["never"]);
    assert_eq!(en(&a[0].held), ["`amount` (what the call gives, `amount`) is 501JPY to 1000JPY"]);
    // some are: the examples are of amounts the flow can give
    let a = asked(&flow(&ranged(">=1JPY <=1_000JPY"), pay), &g);
    assert_eq!(words(&a), ["sometimes"]);
    let Found::Value(Allowance::Sometimes { allowed, denied }) = &a[0].allowance else { unreachable!() };
    assert!(allowed.en.contains("amount: 1JPY to 500JPY") && denied.en.contains("amount: 501JPY to 1000JPY"), "{allowed:?} {denied:?}");
    // the whole range of the input holds nothing
    let a = asked(&flow(INPUTS, pay), &g);
    assert_eq!(words(&a), ["sometimes"]);
    assert!(a[0].held.is_empty(), "{:?}", a[0]);
    // an enum: under the arm of a value, the call gives that value; a value written out is itself
    let g = gate("", "", AMOUNT, "", "  when channel is bank\n");
    let arms = "  match channel
    bank =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = true
    cash =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = false
";
    let a = asked(&flow(INPUTS, arms), &g);
    assert_eq!(words(&a), ["always", "never"]);
    assert_eq!(en(&a[1].held), ["`channel` (what the call gives, `channel`) is `cash`"]);
    let written = "  pay_claim(claimId: claim_id, amount: amount, channel: bank)\n  pay_claim(claimId: claim_id, amount: amount, channel: cash)\n  succeed done = true\n";
    assert_eq!(words(&asked(&flow(INPUTS, written), &g)), ["always", "never"]);
    // the rule's answer on what the call gives: the desk's arm gives up to 500 yen
    let g = gate("", "", AMOUNT, "", "  when amount <= 500JPY\n");
    let a = asked(&flow(INPUTS, BOTH_ARMS), &g);
    assert_eq!(words(&a), ["always", "never"]);
    assert_eq!(en(&a[0].held), ["`who_pays(amount: amount).who` (the flow's `r.who`) is `desk`"]);
    assert!(a[0].rests_on.is_empty());
}

/// An enum, a bool and a value that may be absent of the resource, each the value of the flow a
/// line says it is, with what the arms of a `match` on it leave.
#[test]
fn an_enum_a_bool_and_an_absent_value_of_the_resource() {
    let inputs = "  amount   : money[JPY]  range >=1JPY <=10_000JPY\n  channel  : channel\n  kind     : kind\n  rush     : bool\n  cap      : money[JPY]?  range >=1JPY <=10_000JPY\n";
    let attributes = "    kind : kind\n    rush : bool\n    cap  : money[JPY]?  range >=1JPY <=10_000JPY\n";
    let holds = "  resource Claim from claim_id\n    kind = kind\n    rush = rush\n    cap  = cap\n";
    let pay = "pay_claim(claimId: claim_id, amount: amount, channel: channel)";
    // the enum
    let body = format!("  match kind\n    routine =>\n      {pay}\n    special =>\n      {pay}\n  succeed done = true\n");
    let a = asked(&flow(inputs, &body), &gate("", holds, attributes, "", "  when resource.kind is routine\n"));
    assert_eq!(words(&a), ["always", "never"]);
    assert_eq!(en(&a[1].held), ["`resource.kind` (the flow's `kind`) is `special`"]);
    assert_eq!(a[1].rests_on.iter().map(|(t, l)| (t.en.as_str(), *l)).collect::<Vec<_>>(), [("`kind = kind` under `resource Claim from claim_id`", 13)]);
    // the bool
    let body = format!("  match rush\n    true =>\n      {pay}\n    false =>\n      {pay}\n  succeed done = true\n");
    let a = asked(&flow(inputs, &body), &gate("", holds, attributes, "", "  unless resource.rush\n"));
    assert_eq!(words(&a), ["never", "always"]);
    assert_eq!(en(&a[0].held), ["`resource.rush` (the flow's `rush`) is `true`"]);
    // the value that may be absent: under `none` it is absent, under `some` it is there
    let body = format!("  match cap\n    none =>\n      {pay}\n    some c =>\n      {pay}\n  succeed done = true\n");
    let a = asked(&flow(inputs, &body), &gate("", holds, attributes, "", "  when resource.cap <= 10_000JPY\n"));
    assert_eq!(words(&a), ["never", "always"]);
    assert_eq!(en(&a[0].held), ["`resource.cap` (the flow's `cap`) is absent"]);
    assert_eq!(en(&a[1].held), ["`resource.cap` (the flow's `cap`) is never absent"]);
    // an attribute the lines leave out is one the flow holds nothing of: every value of it
    let body = format!("  match kind\n    routine =>\n      {pay}\n    special => pass\n  succeed done = true\n");
    let a = asked(&flow(inputs, &body), &gate("", holds, attributes, "", "  when resource.kind is routine\n  unless resource.rush\n").replace("    rush = rush\n", ""));
    assert_eq!(words(&a), ["sometimes"]);
    assert!(a[0].unrelated.is_empty() && a[0].unheld.is_empty(), "{:?}", a[0]);
    let Found::Value(Allowance::Sometimes { allowed, denied }) = &a[0].allowance else { unreachable!() };
    assert!(allowed.en.contains("kind: routine, rush: no") && denied.en.contains("kind: routine, rush: yes"), "{allowed:?} {denied:?}");
}

/// What the runs hold that the gate is not asked with is said, and leaves the answer over more
/// combinations.
#[test]
fn what_the_answer_does_not_count() {
    let g = gate("", HOLDS_AMOUNT, AMOUNT, "", "  when resource.amount <= 500JPY\n");
    // the call gives another id than the line's
    let other = BOTH_ARMS.replace("claimId: claim_id", "claimId: other_id");
    let a = asked(&flow(&format!("{INPUTS}  other_id : string\n"), &other), &g);
    assert_eq!(words(&a), ["sometimes", "sometimes"]);
    assert_eq!(a[0].unrelated, ["resource.amount"]);
    assert_eq!(en(&a[0].unheld), ["`resource Claim from claim_id` (line 12) says nothing of this call, which gives `other_id` as the resource's id"]);
    // the rule was given a value the gate has no word for
    let total = BOTH_ARMS.replace("k(amount: amount)", "k(amount: total)");
    let f = flow(&format!("{INPUTS}  total    : money[JPY]  range >=1JPY <=10_000JPY\n"), &total);
    let a = asked(&f, &g);
    assert_eq!(words(&a), ["sometimes", "sometimes"]);
    assert_eq!(en(&a[0].unheld), [format!("what a `match` leaves of the flow's `r.who` (the answer of the call of the rule on line {}) is not counted: `total`, which the rule was given, is no input of the action, and no `resource` line under the workflow says an attribute is it", line_of(&f, "let r = k("))]);
    assert!(a[0].unrelated.is_empty() && a[0].held.is_empty(), "{:?}", a[0]);
    // the variable the rule was given is given another value before the call
    let again = "  let a = amount
  let r = k(amount: a)
  match r.who
    desk =>
      let a = other
      pay_claim(claimId: claim_id, amount: a, channel: channel)
      succeed done = true
    person =>
      succeed done = false
";
    let f = flow(&format!("{INPUTS}  other    : money[JPY]  range >=1JPY <=10_000JPY\n"), again);
    let a = asked(&f, &g.replace("    amount = amount\n", "    amount = a\n"));
    assert_eq!(words(&a), ["sometimes"]);
    assert_eq!(en(&a[0].unheld), [format!("what a `match` leaves of the flow's `r.who` (the answer of the call of the rule on line {}) is not counted: `a`, which the rule was given, can be given another value on the way to this call, or is no input, variable or constant", line_of(&f, "let r = k("))]);
    // the value a line names has none yet on some run that reaches the call
    let unset = "  match channel
    bank =>
      let q = quote(claimId: claim_id)
    cash => pass
  pay_claim(claimId: claim_id, amount: amount, channel: channel)
  succeed done = true
";
    let a = asked(&flow(INPUTS, unset), &g.replace("    amount = amount\n", "    amount = q.total\n"));
    assert_eq!(words(&a), ["sometimes"]);
    assert_eq!(en(&a[0].unheld), ["`amount = q.total` (line 13) is not counted: `q` may have no value yet where the flow makes this call"]);
    assert!(a[0].unheld.iter().all(|t| !t.ja.is_empty()));
}

/// A call under a loop is asked with the item of the round: the line names the item's fields.
#[test]
fn a_call_under_a_loop() {
    let g = gate("", "  resource Claim from c.id\n    amount = c.total\n", AMOUNT, "", "  when resource.amount <= 500JPY\n");
    let body = "  for c in claims at most 10
    let r = k(amount: c.total)
    match r.who
      desk =>
        pay_claim(claimId: c.id, amount: c.total, channel: channel)
      person =>
        pass
  succeed done = true
";
    let a = asked(&flow("  claims   : list[Claim]\n  channel  : channel\n", body), &g);
    assert_eq!(words(&a), ["always"]);
    assert_eq!(a[0].rests_on.iter().map(|(t, _)| t.en.as_str()).collect::<Vec<_>>(), ["`amount = c.total` under `resource Claim from c.id`"]);
}

/// An answer that is the same over every combination of the gate rests on nothing, and one the
/// call's own values decide does not rest on the gate's lines.
#[test]
fn an_answer_that_rests_on_no_line() {
    // allowed whatever the claim holds
    let a = asked(&flow(INPUTS, BOTH_ARMS), &gate("", HOLDS_AMOUNT, AMOUNT, "", ""));
    assert_eq!(words(&a), ["always", "always"]);
    assert!(a.iter().all(|x| x.rests_on.is_empty() && x.held.is_empty() && x.unrelated.is_empty()), "{a:?}");
    // the permit reads the input, which the call gives: the line of the claim's amount is beside it
    let a = asked(&flow(INPUTS, BOTH_ARMS), &gate("", HOLDS_AMOUNT, AMOUNT, "", "  when amount <= 500JPY\n"));
    assert_eq!(words(&a), ["always", "never"]);
    assert!(a.iter().all(|x| x.rests_on.is_empty()), "{a:?}");
    // Cedar written by hand has no word for what a flow holds: the port's default is the answer
    // over every combination, and a workflow the gate does not name is never allowed
    let t = ritsu_testkit::TempDir::new("at-default");
    std::fs::create_dir_all(t.path().join("gates")).unwrap();
    std::fs::write(t.path().join("gates/desk.gate"), gate("", "", AMOUNT, "", "")).unwrap();
    std::fs::create_dir_all(t.path().join("flows")).unwrap();
    std::fs::write(t.path().join("flows/pay.flow"), flow(INPUTS, BOTH_ARMS)).unwrap();
    std::fs::create_dir_all(t.path().join("api")).unwrap();
    std::fs::write(t.path().join("api/desk.json"), API).unwrap();
    std::fs::create_dir_all(t.path().join("rules")).unwrap();
    std::fs::write(t.path().join("rules/who.rule"), WHO).unwrap();
    let e = Engine::new(common::joined());
    let call = ritsu_ports::OperationCall::new(1, "pay_claim", ritsu_base::naming::Name::file(ritsu_base::naming::Tool::Openapi, "api/desk.json").with("operation", "payClaim"), None);
    let root = std::fs::canonicalize(t.path()).unwrap();
    assert_eq!(word(&e.allowed_at(&root, "gates/desk.gate", "pay_claim", "pay", &call).unwrap()), "always");
    assert_eq!(word(&e.allowed_at(&root, "gates/desk.gate", "pay_claim", "audit", &call).unwrap()), "never");
    assert!(e.allowed_at(&root, "gates/desk.gate", "close_claim", "pay", &call).is_err());
}

/// An arm inside an arm holds what both leave; an arm that leaves the flow leaves the runs of the
/// other arms to what comes after the `match`; and where the runs of two arms come to one call,
/// the call is asked with what both can hold.
#[test]
fn arms_inside_arms_and_arms_that_leave() {
    let uses = "use rule k from \"../rules/who.rule\"\n";
    let context = "    who = k(amount: resource.amount).who\n";
    let g = gate(uses, HOLDS_AMOUNT, AMOUNT, context, "  when who is desk\n  when channel is bank\n");
    // the desk's arm, and in it the bank's and the cash's
    let nested = "  let r = k(amount: amount)
  match r.who
    desk =>
      match channel
        bank =>
          pay_claim(claimId: claim_id, amount: amount, channel: channel)
          succeed done = true
        cash =>
          pay_claim(claimId: claim_id, amount: amount, channel: channel)
          succeed done = false
    person => succeed done = false
";
    let a = asked(&flow(INPUTS, nested), &g);
    assert_eq!(words(&a), ["always", "never"]);
    assert_eq!(en(&a[0].held), ["`channel` (what the call gives, `channel`) is `bank`", "`k(amount: resource.amount).who` (the flow's `r.who`) is `desk`"], "{:?}", a[0]);
    // the cash's arm is denied whoever pays: the answer says the channel alone, rests on no line
    // of the gate, and leaves nothing unsaid
    assert_eq!(en(&a[1].held), ["`channel` (what the call gives, `channel`) is `cash`"], "{:?}", a[1]);
    assert!(a[1].rests_on.is_empty() && a[1].unrelated.is_empty() && a[1].unheld.is_empty(), "{:?}", a[1]);
    // the person's arm fails the run: only the runs of the desk's arm come to the call after it
    let leaves = "  let r = k(amount: amount)
  match r.who
    desk => pass
    person => fail LeftToAPerson \"A person pays this claim\"
  match channel
    bank => pass
    cash => fail NoCash \"The desk pays by bank\"
  pay_claim(claimId: claim_id, amount: amount, channel: channel)
  succeed done = true
";
    let a = asked(&flow(INPUTS, leaves), &g);
    assert_eq!(words(&a), ["always"]);
    assert_eq!(a[0].held.len(), 2, "{:?}", a[0]);
    // the runs of both of the bank's and the cash's arms come to the one call: the channel is
    // either, and the answer is the gate's over both
    let joins = "  let r = k(amount: amount)
  match r.who
    desk => pass
    person => fail LeftToAPerson \"A person pays this claim\"
  match channel
    bank => pass
    cash => pass
  pay_claim(claimId: claim_id, amount: amount, channel: channel)
  succeed done = true
";
    let a = asked(&flow(INPUTS, joins), &g);
    assert_eq!(words(&a), ["sometimes"]);
    assert_eq!(en(&a[0].held), ["`k(amount: resource.amount).who` (the flow's `r.who`) is `desk`"], "{:?}", a[0]);
    let Found::Value(Allowance::Sometimes { allowed, denied }) = &a[0].allowance else { unreachable!() };
    assert!(allowed.en.contains("channel: bank") && denied.en.contains("channel: cash"), "{allowed:?} {denied:?}");
    assert!(!denied.en.contains("who: person"), "{denied:?}");
}

/// An attribute that points to an entity is none a line can say a value of the flow for: an answer
/// that reads it leaves nothing unsaid, and its examples are the gate's own.
#[test]
fn an_attribute_that_points_to_an_entity() {
    let attributes = "    amount   : money[JPY]  range >=1JPY <=10_000JPY\n    asked_by : Clerk\n    checked  : Clerk\n";
    let g = gate("", "", attributes, "", "  unless resource.asked_by is resource.checked\n").replace("\nworkflow pay from", "\nprincipal Clerk\n  description \"A member of the staff\"\n\nworkflow pay from");
    let a = asked(&flow(INPUTS, BOTH_ARMS), &g);
    assert_eq!(words(&a), ["sometimes", "sometimes"]);
    assert!(a.iter().all(|x| x.unrelated.is_empty() && x.unheld.is_empty() && x.rests_on.is_empty()), "{a:?}");
}

/// A `resource … from` line under the workflow says what the flow holds of the resource at every
/// call that gives its `from` value as the resource's id: E212 holds the value the line names to
/// the attribute's range at each such call, by what the runs that reach that call last put in it,
/// and says which calls such a value reaches. A value put in on a way that reaches none of them is
/// not counted.
#[test]
fn e212_holds_the_value_to_the_attribute_at_each_call_the_line_speaks_of() {
    let g = gate("", "  resource Claim from claim_id\n    amount = a\n", "    amount : money[JPY]  range >=1JPY <=5_000JPY\n", "", "  when resource.amount <= 500JPY\n");
    let inputs = "  amount   : money[JPY]  range >=1JPY <=5_000JPY\n  channel  : channel\n  other    : money[JPY]  range >=1JPY <=10_000JPY\n";
    let pay = "  pay_claim(claimId: claim_id, amount: a, channel: channel)\n";
    // the larger amount is put in after the call, and on a way that leaves the flow
    let after = format!("  let a = amount\n{pay}  let a = other\n  succeed done = true\n");
    assert_eq!(checked(&flow(inputs, &after), &g).0, Vec::<&str>::new());
    let leaves = format!("  let a = amount\n  match channel\n    bank =>\n      let a = other\n      succeed done = false\n    cash => pass\n{pay}  succeed done = true\n");
    assert_eq!(checked(&flow(inputs, &leaves), &g).0, Vec::<&str>::new());
    // the second call is given it: E212, which says the call
    let twice = format!("  let a = amount\n{pay}  let a = other\n{pay}  succeed done = true\n");
    let f = flow(inputs, &twice);
    let (codes, text) = checked(&f, &g);
    assert_eq!(codes, ["E212"], "{text}");
    let second = f.lines().enumerate().filter(|(_, l)| l.contains("pay_claim(claimId: claim_id, amount: a")).nth(1).unwrap().0 + 1;
    assert!(text.contains(&format!("Such a value reaches the call of `pay_claim` on line {second} in the flow.")), "{text}");
    assert!(text.contains(&format!("フローの {second} 行目の `pay_claim` の呼び出しに、そういう値が届きます。")), "{text}");
    // a call that gives another id is none the line speaks of
    let other_id = format!("  let a = amount\n{pay}  let a = other\n  pay_claim(claimId: other_id, amount: a, channel: channel)\n  succeed done = true\n");
    assert_eq!(checked(&flow(&format!("{inputs}  other_id : string\n"), &other_id), &g).0, Vec::<&str>::new());
}

/// A `resource … from` line under the workflow that no call of the flow speaks of (none gives its
/// `from` value as the id of a resource of its type, to an operation an action of the gate guards)
/// holds at no call: W202 says so, with why — the calls give the id another value, the flow calls
/// none of the operations, no action takes a resource of the line's type from an argument, or the
/// one that takes it does not say which argument gives its id — and what to write.
#[test]
fn a_line_no_call_speaks_of() {
    let pay = "  pay_claim(claimId: claim_id, amount: amount, channel: channel)\n  succeed done = true\n";
    let inputs = format!("{INPUTS}  other_id : string\n");
    // the line the call speaks of: nothing
    assert_eq!(checked(&flow(&inputs, pay), &gate("", HOLDS_AMOUNT, AMOUNT, "", "")).0, Vec::<&str>::new());
    // the calls give the id another value
    let g = gate("", "  resource Claim from other_id\n    amount = amount\n", AMOUNT, "", "");
    let f = flow(&inputs, pay);
    let (codes, text) = checked(&f, &g);
    assert_eq!(codes, ["W202"], "{text}");
    let line = line_of(&f, "  pay_claim(claimId: claim_id");
    for said in [
        "The line speaks of no call: no call of the workflow `pay` (`../flows/pay.flow`) gives `other_id` as the id of a `Claim`".to_string(),
        format!("The calls of the operations the actions that take a `Claim` guard (`pay_claim`) give the id another value (`pay_claim` on line {line} gives `claim_id` as `claimId`)."),
        "Write the value the calls give as the id: `resource Claim from claim_id`.".to_string(),
        "この行が当たる呼び出しがありません。ワークフロー `pay`（`../flows/pay.flow`）のどの呼び出しも、`other_id` を `Claim` の ID として渡しません".to_string(),
        format!("`Claim` を受け取る action（`pay_claim`）が守る操作の呼び出しは、ID の引数に別の値を渡します（{line} 行目の `pay_claim` は `claimId` に `claim_id`）。"),
        "呼び出しが ID として渡す値を書いてください（`resource Claim from claim_id`）。".to_string(),
    ] {
        assert!(text.contains(&said), "{said}\n{text}");
    }
    // the flow calls none of the operations
    let quotes = "  let q = quote(claimId: claim_id)\n  succeed done = true\n";
    let (codes, text) = checked(&flow(&inputs, quotes), &gate("", HOLDS_AMOUNT, AMOUNT, "", ""));
    assert_eq!(codes, ["W202"], "{text}");
    assert!(text.contains("The flow calls none of the operations the actions that take a `Claim` guard (`pay_claim`)."), "{text}");
    assert!(text.contains("フローは、`Claim` を受け取る action（`pay_claim`）が守る操作を、どれも呼びません。"), "{text}");
    // no action takes a resource of the line's type from an argument
    let g = gate("", "  resource Folder from claim_id\n", AMOUNT, "", "").replace("\nresource Claim\n", "\nresource Folder\n  description \"A folder\"\n\nresource Claim\n");
    let (codes, text) = checked(&flow(&inputs, pay), &g);
    assert_eq!(codes, ["W202"], "{text}");
    assert!(text.contains("No action of the gate takes a `Folder` from an argument: `pay_claim` takes `Claim` from `claimId`."), "{text}");
    assert!(text.contains("`Folder` を引数から受け取る action がありません。`pay_claim` は `Claim` を `claimId` から受け取ります。"), "{text}");
    // the action that takes one says of no argument that it gives the id
    let g = gate("", HOLDS_AMOUNT, AMOUNT, "", "").replace("  resource Claim from claimId\n", "  resource Claim\n");
    let (codes, text) = checked(&flow(&inputs, pay), &g);
    assert_eq!(codes, ["W202"], "{text}");
    assert!(text.contains("`pay_claim` takes a `Claim`, but does not say which argument gives its id (`resource Claim from <argument>`)."), "{text}");
    assert!(text.contains("Write which argument of the action gives the resource's id: `resource Claim from <argument>`."), "{text}");
    assert!(text.contains("`pay_claim` は `Claim` を受け取りますが、どの引数が resource の ID かを書いていません（`resource Claim from <引数>`）。"), "{text}");
    // an operation the contract does not have leaves the calls of the action unknown: E202's
    let g = gate("", HOLDS_AMOUNT, AMOUNT, "", "").replace("  guards desk payClaim\n", "  guards desk payNoClaim\n");
    let (codes, text) = checked(&flow(&inputs, pay), &g);
    assert!(codes.contains(&"E202") && !codes.contains(&"W202"), "{text}");
    // a value the flow does not have is E212's, not this
    let g = gate("", "  resource Claim from no_such_id\n", AMOUNT, "", "");
    assert_eq!(checked(&flow(&inputs, pay), &g).0, ["E212"]);
}

/// A number written out where the task takes an amount is that amount, in the unit of the task's
/// parameter (as dandori's check takes it, and the code it writes passes it): the call is asked
/// with it.
#[test]
fn a_number_written_out_as_an_amount() {
    let g = gate("", "", AMOUNT, "", "  when amount <= 500JPY\n");
    let pay = |n: &str| format!("  pay_claim(claimId: claim_id, amount: {n}, channel: channel)\n  succeed done = true\n");
    for (n, word) in [("300", "always"), ("800", "never")] {
        let a = asked(&flow(INPUTS, &pay(n)), &g);
        assert_eq!(words(&a), [word], "{n}: {a:?}");
        assert!(a[0].unheld.is_empty() && a[0].unrelated.is_empty(), "{n}: {:?}", a[0]);
    }
    let a = asked(&flow(INPUTS, &pay("300")), &g);
    assert_eq!(en(&a[0].held), ["`amount` (what the call gives, `300`) is 300JPY"]);
    // a number of another unit than the input's is not read as one: not counted, and said so
    let yen = flow(INPUTS, &pay("300"));
    let pounds = yen.replace("amount: money[JPY] range >=1JPY <=10_000JPY, channel: channel)", "amount: money[GBP] range >=1GBP <=10_000GBP, channel: channel)");
    assert_ne!(pounds, yen);
    let a = asked(&pounds, &g);
    assert_eq!(words(&a), ["sometimes"], "{a:?}");
    assert!(a[0].held.is_empty(), "{:?}", a[0]);
    assert_eq!(en(&a[0].unheld), ["the flow's `300` is not of the type of the gate's `amount`"]);
}

/// A number written out where the task takes a rate in steps (`rate[step 0.1%]`) is that many
/// steps, as the code dandori writes passes it: `fee: 5` is 0.5%, and the call is asked with it.
/// Written with its unit (`fee: 0.5%`), it is the same 5 steps, quoted as written. A gate whose
/// input counts other steps does not read it, and says so.
#[test]
fn a_number_written_out_as_a_rate_in_steps() {
    // the operation takes a fee too, an integer up to 100: steps of 0.1%, up to 10%
    let api = API
        .replace("\"required\": [\"amount\", \"channel\"]", "\"required\": [\"amount\", \"channel\", \"fee\"]")
        .replace("\"enum\": [\"bank\", \"cash\"] } } } } } },", "\"enum\": [\"bank\", \"cash\"] },\n          \"fee\": { \"type\": \"integer\", \"minimum\": 0, \"maximum\": 100 } } } } } },");
    assert_eq!(api.matches("\"fee\"").count(), 2, "{api}");
    let pay = |n: &str| {
        flow(INPUTS, &format!("  pay_claim(claimId: claim_id, amount: amount, channel: channel, fee: {n})\n  succeed done = true\n"))
            .replace("channel: channel) -> Paid", "channel: channel, fee: rate[step 0.1%] range >=0 <=100) -> Paid")
    };
    let fee_gate = |step: &str, conds: &str| gate("", "", AMOUNT, "", conds).replace("    channel : channel\n", &format!("    channel : channel\n    fee     : rate[step {step}]  range >=0% <=10%\n"));
    let ask = |f: &str, g: &str| -> Vec<AllowedAt> {
        let t = ritsu_testkit::TempDir::new("at-fee");
        let root = laid_out(&t, f, g);
        std::fs::write(root.join("api/desk.json"), &api).unwrap();
        let p = common::ports();
        let ports = ritsu_ports::Ports { rules: p.rules.clone(), dates: p.dates.clone(), books: p.books.clone() };
        let (_, calls) = p.flows.operation_calls(&root, "flows/pay.flow", &ports).unwrap_or_else(|s| panic!("the flow: {s:?}\n{f}"));
        let e = Engine::new(common::joined());
        calls.iter().filter(|c| c.task == "pay_claim").map(|c| e.allowed_at(&root, "gates/desk.gate", "pay_claim", "pay", c).unwrap_or_else(|s| panic!("the gate: {s:?}\n{g}"))).collect()
    };
    let g = fee_gate("0.1%", "  when fee <= 1%\n");
    // 5 steps, 0.5%, which the gate allows; 50 steps, 5%, which it does not
    for (n, word) in [("5", "always"), ("50", "never")] {
        let a = ask(&pay(n), &g);
        assert_eq!(words(&a), [word], "{n}: {a:?}");
        assert!(a[0].unheld.is_empty() && a[0].unrelated.is_empty(), "{n}: {:?}", a[0]);
    }
    let a = ask(&pay("5"), &g);
    assert_eq!(en(&a[0].held), ["`fee` (what the call gives, `5`) is 0.5%"]);
    assert_eq!(a[0].held.iter().map(|t| t.ja.as_str()).collect::<Vec<_>>(), ["`fee`（呼び出しが渡す `5`）は 0.5%"]);
    // the same fees written with their unit: the same steps, so the same answers, quoted as written
    for (n, word) in [("0.5%", "always"), ("5%", "never")] {
        let a = ask(&pay(n), &g);
        assert_eq!(words(&a), [word], "{n}: {a:?}");
        assert!(a[0].unheld.is_empty() && a[0].unrelated.is_empty(), "{n}: {:?}", a[0]);
    }
    let a = ask(&pay("0.5%"), &g);
    assert_eq!(en(&a[0].held), ["`fee` (what the call gives, `0.5%`) is 0.5%"]);
    assert_eq!(a[0].held.iter().map(|t| t.ja.as_str()).collect::<Vec<_>>(), ["`fee`（呼び出しが渡す `0.5%`）は 0.5%"]);
    // a gate that counts the fee in steps of 1% does not read the flow's steps of 0.1% as its own
    let a = ask(&pay("5"), &fee_gate("1%", "  when fee <= 1%\n"));
    assert_eq!(words(&a), ["sometimes"], "{a:?}");
    assert!(a[0].held.is_empty(), "{:?}", a[0]);
    assert_eq!(en(&a[0].unheld), ["the flow's `5` is not of the type of the gate's `fee`"]);
}

/// Who pays a claim by its amount and its fee: the desk up to 500 yen at a fee up to 1%, a person
/// otherwise.
const BY_FEE: &str = "rule who_pays_by_fee(f) v1
description \"Who pays a claim: the desk up to 500 yen at a fee up to 1%, a person otherwise\"

enum payer = desk \"Desk\" | person \"Person\"

inputs
  amount : money[JPY]  range >=1JPY <=10_000JPY
  fee    : rate[step 0.1%]  range >=0% <=10%

outputs
  who : payer

table t
| amount   | fee  | -> who |
| <=500JPY | <=1% | desk   |
| <=500JPY | >1%  | person |
| >500JPY  | -    | person |

examples
| amount | fee  | -> who |
| 500JPY | 1%   | desk   |
| 500JPY | 1.1% | person |
| 501JPY | 0%   | person |
";

/// A number written out where a rule takes a rate in steps is that many steps of it, as the code
/// passes it: the gate, which has no word for the fee, reads the rule's answer with `fee: 0.5%`, so
/// the desk's arm pays only claims up to 500 yen, and the person's only those above.
#[test]
fn a_number_written_out_to_a_rule_as_a_rate_in_steps() {
    let body = "  let r = f(amount: amount, fee: 5)
  match r.who
    desk =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = true
    person =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = false
";
    let f = flow(INPUTS, body).replace("use rule k from", "use rule f from \"../rules/fee.rule\"\nuse rule k from");
    let g = gate("", HOLDS_AMOUNT, AMOUNT, "", "  when resource.amount <= 500JPY\n");
    let t = ritsu_testkit::TempDir::new("at-rule-fee");
    let root = laid_out(&t, &f, &g);
    std::fs::write(root.join("rules/fee.rule"), BY_FEE).unwrap();
    let p = common::ports();
    let ports = ritsu_ports::Ports { rules: p.rules.clone(), dates: p.dates.clone(), books: p.books.clone() };
    let (_, calls) = p.flows.operation_calls(&root, "flows/pay.flow", &ports).unwrap_or_else(|s| panic!("the flow: {s:?}\n{f}"));
    let e = Engine::new(common::joined());
    let a: Vec<AllowedAt> = calls.iter().filter(|c| c.task == "pay_claim").map(|c| e.allowed_at(&root, "gates/desk.gate", "pay_claim", "pay", c).unwrap_or_else(|s| panic!("the gate: {s:?}\n{g}"))).collect();
    assert_eq!(words(&a), ["always", "never"], "{a:?}");
    assert!(a.iter().all(|x| x.unheld.is_empty()), "{a:?}");
}

/// A value the rule was given that the gate has no word for — here a task's answer — is read by
/// what the flow says it can be where it calls the rule, when that is known (a number with both
/// ends): what a `match` leaves of the answer is counted over it, as an input of the action the
/// gate does not have. One with no range is not counted, as before.
#[test]
fn a_value_the_gate_has_no_word_for_by_what_the_flow_says_it_can_be() {
    let g = gate("", HOLDS_AMOUNT, AMOUNT, "", "  when resource.amount <= 500JPY\n");
    let body = "  let q = quote(claimId: claim_id)
  let r = late(amount: amount, days: q.days)
  match r.who
    desk =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = true
    person =>
      pay_claim(claimId: claim_id, amount: amount, channel: channel)
      succeed done = false
";
    let f = flow(INPUTS, body).replace("use rule k from", "use rule late from \"../rules/late.rule\"\nuse rule k from");
    // the desk's arm: up to 500 yen within a week; the person's: either above 500 yen, or late
    let a = asked(&f, &g);
    assert_eq!(words(&a), ["always", "sometimes"], "{a:?}");
    assert!(a[0].unheld.is_empty(), "{:?}", a[0]);
    // nothing says how many days the quote can answer
    let open = f.replace("days  : int  range >=0 <=30", "days  : int");
    assert_ne!(open, f);
    let a = asked(&open, &g);
    assert_eq!(words(&a), ["sometimes", "sometimes"], "{a:?}");
    let line = line_of(&open, "let r = late(");
    assert_eq!(en(&a[0].unheld), [format!("what a `match` leaves of the flow's `r.who` (the answer of the call of the rule on line {line}) is not counted: `q.days`, which the rule was given, is no input of the action, and no `resource` line under the workflow says an attribute is it")]);
}
