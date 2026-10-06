//! X14 over a project with a real map (DESIGN 16.8): sakai answers the map and the context of each
//! file (`Maps`), and what dandori would say a flow sends (`Flows::sends`) is given by the test, so
//! that every way a secret can go is held to sakai's own answers. The flow, in Ordering, sends the
//! card's number (marked by Payments, `debug_redact` in `payments/v1/card.proto`) to Payments
//! itself, to Notices (which goes separate ways from Payments), to a file the map does not cover,
//! and to Notices again under `discloses`; and a token its own `.flow` marks to Ordering. The same
//! project in Japanese names, and a map that does not pass sakai's check (W905).

use ritsu_base::text::Lang;
use ritsu_cross::Borders;
use ritsu_ports::{Crossings, Destination, Flows, Ports, RuleCall, Rules, Said, Secret, Send};
use ritsu_project::{Joined, Project};
use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The names of one version of the project.
struct Names {
    map: &'static str,
    payments: &'static str,
    ordering: &'static str,
    notices: &'static str,
}

const EN: Names = Names { map: "Shop(shop)", payments: "Payments(payments)", ordering: "Ordering(ordering)", notices: "Notices(notices)" };
const JA: Names = Names { map: "店(shop)", payments: "決済(payments)", ordering: "受注(ordering)", notices: "通知(notices)" };

/// The name a relationship is written with: `Payments(payments)` is `Payments`.
fn short(n: &str) -> &str {
    n.split('(').next().unwrap()
}

fn project(n: &Names, map_ok: bool) -> TempDir {
    let t = TempDir::new("egress-map");
    let missing = if map_ok { "" } else { "use context \"contexts/missing.ctx\"\n" };
    t.write("shop.ctx", format!("map {} v1\ndescription \"Three contexts of a small shop\"\n\nuse context \"contexts/payments.ctx\"\nuse context \"contexts/ordering.ctx\"\nuse context \"contexts/notices.ctx\"\n{missing}\ncovers \"payments\", \"ordering\", \"notices\"\n", n.map));
    t.write("contexts/payments.ctx", format!("context {} v1\ndescription \"Charges the customer's card\"\nowner \"Payments team\"\n\nowns\n  dir \"../payments\"\n\npartnership with {}\n", n.payments, short(n.ordering)));
    t.write("contexts/ordering.ctx", format!("context {} v1\ndescription \"Takes orders and sees them paid\"\nowner \"Ordering team\"\n\nowns\n  dir \"../ordering\"\n\npartnership with {}\n", n.ordering, short(n.payments)));
    t.write("contexts/notices.ctx", format!("context {} v1\ndescription \"Writes to the customer\"\nowner \"Customer care team\"\n\nowns\n  dir \"../notices\"\n\nseparate ways from {}\n", n.notices, short(n.payments)));
    t.write("payments/v1/card.proto", "syntax = \"proto3\";\n\npackage payments.v1;\n\nmessage Card {\n  string id = 1;\n  string number = 2 [debug_redact = true];\n}\n");
    for f in ["payments/api/charges.yaml", "ordering/api/orders.yaml", "notices/api/notices.yaml", "tools/report.yaml"] {
        t.write(f, "openapi: 3.1.0\ninfo:\n  title: x\n  version: 1.0.0\npaths: {}\n");
    }
    t.write("ordering/checkout.flow", FLOW);
    t
}

/// The flow: five tasks, called on lines 30 to 34 (the lines the sends below give).
const FLOW: &str = "workflow checkout v1
description \"Takes an order and its payment\"

inputs
  order : string
  card  : string

task charge(card: string)
  lambda \"arn:aws:lambda:us-east-1:123456789012:function:charge\"
  key

task notify(card: string)
  lambda \"arn:aws:lambda:us-east-1:123456789012:function:notify\"
  key

task report(card: string)
  lambda \"arn:aws:lambda:us-east-1:123456789012:function:report\"
  key

task notify_again(card: string)
  lambda \"arn:aws:lambda:us-east-1:123456789012:function:notify-again\"
  key

task place(order: string)
  lambda \"arn:aws:lambda:us-east-1:123456789012:function:place\"
  key

flow
  # the sends the test gives are on the lines of these calls
  charge(card: card)
  notify(card: card)
  report(card: card)
  notify_again(card: card)
  place(order: order)
";

/// What dandori would say `ordering/checkout.flow` sends, its paths as dandori reaches them
/// (absolute).
struct Sending {
    root: PathBuf,
}

impl Sending {
    fn send(&self, line: usize, task: &str, to: &str, secrets: Vec<(&str, Secret)>, disclosed: Vec<(&str, &str)>) -> Send {
        Send {
            line,
            task: task.into(),
            to: Destination::File(self.root.join(to)),
            secrets: secrets.into_iter().map(|(p, s)| (p.to_string(), s)).collect(),
            disclosed: disclosed.into_iter().map(|(p, w)| (p.to_string(), w.to_string())).collect(),
        }
    }

    fn card(&self) -> Secret {
        Secret { shown: "card.number".into(), marked_in: self.root.join("payments/v1/card.proto"), line: 7, mark: "debug_redact = true".into() }
    }
}

impl Flows for Sending {
    fn rule_calls(&self, _file: &Path, _rules: Rc<dyn Rules>) -> Result<Vec<RuleCall>, Vec<Said>> {
        Ok(Vec::new())
    }
    fn crossings(&self, _file: &Path, _ports: &Ports) -> Result<Crossings, Vec<Said>> {
        Ok(Crossings::default())
    }
    fn sends(&self, file: &Path, _ports: &Ports) -> Result<Vec<Send>, Vec<Said>> {
        if !file.ends_with("ordering/checkout.flow") {
            return Ok(Vec::new());
        }
        let token = Secret { shown: "token".into(), marked_in: self.root.join("ordering/checkout.flow"), line: 5, mark: "secret".into() };
        Ok(vec![
            self.send(30, "charge", "payments/api/charges.yaml", vec![("card", self.card())], vec![]),
            self.send(31, "notify", "notices/api/notices.yaml", vec![("card", self.card())], vec![]),
            self.send(32, "report", "tools/report.yaml", vec![("card", self.card())], vec![]),
            self.send(33, "notify_again", "notices/api/notices.yaml", vec![("card", self.card())], vec![("card", "the customer asked for the number on the notice")]),
            self.send(34, "place", "ordering/api/orders.yaml", vec![("token", token)], vec![]),
        ])
    }
}

/// X14 over the project at `t`, in `lang`: the text of what it says, with the root as the run
/// writes it taken out, and the borders.
fn run(t: &TempDir, lang: Lang) -> (String, Borders, Vec<String>) {
    let root = t.path().to_string_lossy().to_string();
    let p = Project::load(std::slice::from_ref(&root), Some(&root)).expect("the project loads");
    let joined = Joined::new();
    let flows = Sending { root: t.path().to_path_buf() };
    let mut b = Borders::default();
    let found = ritsu_cross::egress::check_with(&p, &flows, &*joined.maps(), &joined.ports(), lang, &mut b);
    let shown = p.shown.path("ordering/checkout.flow");
    let prefix = shown.strip_suffix("ordering/checkout.flow").unwrap().to_string();
    let text: String = found.iter().map(|f| f.text.replace(&prefix, "")).collect();
    (text, b, found.iter().map(|f| f.code.clone()).collect())
}

#[test]
fn a_secret_goes_where_the_map_says() {
    let mut failures = Vec::new();
    for (names, tag) in [(EN, "en"), (JA, "ja")] {
        let t = project(&names, true);
        for (lang, l) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let (text, b, codes) = run(&t, lang);
            // held: to Payments itself, under discloses, the flow's own mark to Ordering; failed:
            // to Notices, to a file the map does not cover
            assert_eq!(b, Borders { held: 3, failed: 2, undecided: 0 }, "{tag} {l}:\n{text}");
            assert_eq!(codes, ["E905", "E905"], "{tag} {l}:\n{text}");
            if let Err(e) = ritsu_testkit::golden::check(&Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/egress/{tag}.{l}.txt")), &text) {
                failures.push(e);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_map_that_does_not_pass_leaves_it_undecided() {
    let t = project(&EN, false);
    let (text, b, codes) = run(&t, Lang::En);
    assert_eq!(b, Borders { held: 0, failed: 0, undecided: 5 }, "{text}");
    assert_eq!(codes, ["W905"; 5], "{text}");
    if let Err(e) = ritsu_testkit::golden::check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/egress/broken.en.txt"), &text) {
        panic!("{e}");
    }
}

/// A project with no map, and one whose flows send no secret, say nothing and count nothing.
#[test]
fn no_map_no_secret_nothing_said() {
    let t = project(&EN, true);
    std::fs::remove_file(t.path().join("shop.ctx")).unwrap();
    let (text, b, _) = run(&t, Lang::En);
    assert_eq!((text.as_str(), b), ("", Borders::default()));
    // the real dandori says this flow sends nothing
    let t = project(&EN, true);
    let root = t.path().to_string_lossy().to_string();
    let p = Project::load(std::slice::from_ref(&root), Some(&root)).unwrap();
    let crossed = ritsu_cross::check(&p, &Joined::new(), Lang::En);
    assert_eq!(crossed.borders, Borders::default());
    assert!(crossed.findings.iter().all(|f| f.code != "E905" && f.code != "W905"));
}
