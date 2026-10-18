//! `ritsu kaname` and the `.kn` files of `ritsu check` (kaname's DESIGN 11.2): kaname's command,
//! with the languages a module calls joined through the ports (the rules, the dates files, the
//! books), which the binary of kaname's own crate cannot (it says E202; kaname's `tests/cli.rs`).

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn kaname_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kaname")
}

/// `ritsu`, run in `dir`, with no language asked of the environment.
fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_ritsu")).current_dir(dir).env_remove("RITSU_LANG").env_remove("KANAME_LANG").args(args).output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The examples, English first (kaname's DESIGN 17), and what the line that
/// says each passes says of its functions: `ritsu kaname` hands kaname the verdicts of ritsu's checks
/// across the borders at their calls (kaname's DESIGN 4.3, 14.5), which show every one of them, and
/// every function is total.
const EXAMPLES: [(&str, &str); 8] = [
    ("examples/payout/payout.kn", "1 function, total"),
    ("examples/payout/payout.ja.kn", "1 function, total"),
    ("examples/late_payment/late_payment.kn", "1 function, total"),
    ("examples/late_payment/late_payment.ja.kn", "1 function, total"),
    ("examples/media_mail/media_mail.kn", "2 functions, total"),
    ("examples/media_mail/media_mail.ja.kn", "2 functions, total"),
    ("examples/discount_split/discount_split.kn", "1 function, total"),
    ("examples/discount_split/discount_split.ja.kn", "1 function, total"),
];

/// Every example reads its rules, dates file and book in the same process: `ritsu kaname check` passes
/// each.
#[test]
fn ritsu_kaname_checks_the_examples_with_every_language() {
    for (f, fns) in EXAMPLES {
        let (code, out, err) = ritsu_in(&kaname_dir(), &["kaname", "check", f]);
        assert_eq!(code, 0, "{f}: {out}{err}");
        assert_eq!(out, format!("ok {f}: {fns}\n"));
    }
    let (code, out, _) = ritsu_in(&kaname_dir(), &["kaname", "check", "examples/late_payment/late_payment.ja.kn", "--lang", "ja"]);
    assert_eq!((code, out.as_str()), (0, "ok examples/late_payment/late_payment.ja.kn: 関数 1 個、total\n"));
}

/// What is wrong is kaname's to say, as its own command says it; the words after `kaname` are
/// kaname's.
#[test]
fn ritsu_kaname_is_kanames_command() {
    let t = TempDir::new("ritsu-kaname");
    ritsu_testkit::tmp::copy_dir(&kaname_dir().join("examples/late_payment"), t.path());
    let kn = t.path().join("late_payment.kn");
    let src = std::fs::read_to_string(&kn).unwrap();
    std::fs::write(&kn, src.replace("let late = paid_on - due", "let late = paid_on + 30")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["kaname", "check", "late_payment.kn"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[E105]: late_payment.kn:21:14: A date takes no `+`\n"), "{out}");
    let (code, out, _) = ritsu_in(t.path(), &["kaname", "explain", "E105"]);
    assert!(code == 0 && out.starts_with("E105 (error) — The type has no such operation\n"), "{out}");
    let (code, out, _) = ritsu_in(t.path(), &["kaname", "--version"]);
    assert_eq!((code, out), (0, format!("kaname {}\n", env!("CARGO_PKG_VERSION"))));
    let (code, out, _) = ritsu_in(t.path(), &["help", "kaname"]);
    assert!(code == 0 && out.starts_with("kaname "), "{out}");
    // kaname is the ninth of the languages `ritsu --help` lists
    let (_, out, _) = ritsu_in(t.path(), &["--help"]);
    assert!(out.contains("\n  ritsu kaname <command> ...  "), "{out}");
}

/// `ritsu check` on an example: the rule, the dates file, then the modules, each said as its language
/// says it, and the line that sums them up; the day each module gives the date of koyomi is a border
/// between the languages (X6), shown.
#[test]
fn ritsu_check_hands_the_modules_to_kaname() {
    let dir = kaname_dir().join("examples/late_payment");
    let (code, out, err) = ritsu_in(&dir, &["check", ".", "--root", "."]);
    assert_eq!(code, 0, "{out}{err}");
    let modules: Vec<&str> = out.lines().filter(|l| l.contains(".kn: ")).collect();
    assert_eq!(modules, ["ok late_payment.ja.kn: 1 function, total", "ok late_payment.kn: 1 function, total"]);
    assert!(out.ends_with("ritsu check: 6 files (rulec 2, koyomi 2, kaname 2): all pass; borders between the languages: 2 checked, 0 undecided\n"), "{out}");
}

/// `ritsu kaname api` (kaname's DESIGN 11.1), with the verdicts: every call of the payout shown, and
/// the reasons each operation of the ledger can be refused with at the amounts the module gives.
#[test]
fn ritsu_kaname_api_says_the_verdicts() {
    let dir = kaname_dir().join("examples/payout");
    let (code, out, err) = ritsu_in(&dir, &["kaname", "api", "payout.kn", "--root", "."]);
    assert_eq!(code, 0, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let settle = &v["functions"][0];
    assert_eq!(settle["effect"], serde_json::json!({"fails": [], "diverges": false}));
    let outcomes: Vec<&str> = settle["crossings"].as_array().unwrap().iter().flat_map(|c| c["checks"].as_array().unwrap()).map(|c| c["outcome"].as_str().unwrap()).collect();
    assert_eq!(outcomes, ["shown", "shown"]);
    let refusals: Vec<(&str, Vec<&str>)> = v["transfers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["type"].as_str().unwrap(), t["refusals"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect()))
        .collect();
    assert_eq!(refusals, [("ledger.capture.do", vec!["key_conflict"]), ("ledger.processing_fee.do", vec!["insufficient_balance", "key_conflict", "already_refused"])]);
}

/// `ritsu check` runs the `examples` of a module in the reference interpreter, with the rules, the
/// dates files and the books it calls joined (kaname's DESIGN 12, E601): an example whose answer is
/// not the module's fails the project.
#[test]
fn ritsu_check_runs_the_examples_of_a_module() {
    let t = TempDir::new("ritsu-kaname-examples");
    ritsu_testkit::tmp::copy_dir(&kaname_dir().join("examples/late_payment"), t.path());
    let kn = t.path().join("late_payment.kn");
    let src = std::fs::read_to_string(&kn).unwrap();
    std::fs::write(&kn, src.replace("interest: 1150GBPc }", "interest: 1151GBPc }")).unwrap();
    let (code, out, _) = ritsu_in(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[kaname E601]: late_payment.kn:30:3: The example answers Claim { due: 2026-03-31, days_late: 50, compensation: 70GBP, interest: 1150GBPc }, and the answer written is Claim { due: 2026-03-31, days_late: 50, compensation: 70GBP, interest: 1151GBPc }\n"), "{out}");
    let (code, out, _) = ritsu_in(t.path(), &["kaname", "check", "late_payment.kn"]);
    assert_eq!(code, 1);
    assert!(out.starts_with("error[E601]: late_payment.kn:30:3: "), "{out}");
}

/// `ritsu kaname run` and `vectors`: a function run on an input read from standard input, and the
/// vectors of a module, with every language joined.
#[test]
fn ritsu_kaname_runs_and_writes_vectors() {
    use std::io::Write;
    let dir = kaname_dir().join("examples/late_payment");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ritsu"))
        .current_dir(&dir)
        .env_remove("RITSU_LANG")
        .env_remove("KANAME_LANG")
        .args(["kaname", "run", "late_payment.kn", "claim", "--input", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("could not run ritsu");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"invoice": {"id": "inv_1", "invoiced": "2026-03-02", "amount": 100000}, "paid_on": "2026-05-20", "reference_rate": 50}"#)
        .unwrap();
    let o = child.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(out.ends_with("= Claim { due: 2026-03-31, days_late: 50, compensation: 70GBP, interest: 1150GBPc }\n"), "{out}");
    let (code, out, _) = ritsu_in(&dir, &["kaname", "vectors", "late_payment.kn"]);
    assert_eq!(code, 0);
    // the inputs that reach the boundary of `late >= 1` too: a day before it, on it, a day after it
    assert_eq!(out.lines().count(), 19, "{out}");
    assert!(out.lines().all(|l| l.starts_with("{\"function\":\"claim\",\"in\":")), "{out}");
}

/// What a module holds and names (kaname's DESIGN 14.6, `Items` and `References`), as the other
/// languages read it: sakai holds a module's `use rule` to the map — the code a module is made into
/// calls the rule's own code, so a rule inside another context is read only from a shared kernel —
/// and yuen links a requirement to a function of a module, and to a module as what verifies it.
#[test]
fn sakai_and_yuen_read_what_a_module_holds_and_names() {
    let t = TempDir::new("ritsu-kaname-map");
    let example = kaname_dir().join("examples/payout");
    t.write("fees/card_fee.rule", std::fs::read(example.join("rules/card_fee.rule")).unwrap());
    // the copy of the document the rule cites, which rulec holds it to
    ritsu_testkit::tmp::copy_dir(&example.join("rules/sources"), &t.path().join("fees/sources"));
    t.write("payouts/ledger.book", std::fs::read(example.join("books/ledger.book")).unwrap());
    let kn = std::fs::read_to_string(example.join("payout.kn")).unwrap().replace("\"rules/card_fee.rule\"", "\"../fees/card_fee.rule\"").replace("\"books/ledger.book\"", "\"ledger.book\"");
    t.write("payouts/payout.kn", &kn);
    t.write("market.ctx", "map Market(market) v1\ndescription \"A marketplace that takes card payments for its sellers\"\n\nuse context \"contexts/fees.ctx\"\nuse context \"contexts/payouts.ctx\"\n\ncovers \".\"\n");
    let fees = "context Fees(fees) v1\ndescription \"Works out the processor's fee on a card payment\"\nowner \"Payments team\"\n\nowns\n  dir \"../fees\"\n\npublished language rulec.card_fee.v1\n  rulec \"../fees/card_fee.rule\"\n";
    let payouts = "context Payouts(payouts) v1\ndescription \"Pays the sellers what their card payments come to\"\nowner \"Marketplace team\"\n\nowns\n  dir \"../payouts\"\n\nupstream Fees conformist\n  through rulec.card_fee.v1\n";
    t.write("contexts/fees.ctx", fees);
    t.write("contexts/payouts.ctx", payouts);
    let (code, out, _) = ritsu_in(t.path(), &["sakai", "check", "market.ctx"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with("error[E202]: payouts/payout.kn:4:1: The file payouts/payout.kn of Payouts refers to fees/card_fee.rule (use rule), which is inside Fees\n"), "{out}");
    assert!(out.contains("  = The code a module of kaname is made into calls the code made of the rules, the dates and the other modules it reads: the thing itself, inside Fees. One across the boundary is read from the two's shared kernel.\n"), "{out}");
    // the rule in the two's shared kernel: the crossing is counted as kaname's
    t.write("contexts/fees.ctx", format!("{fees}\nshared kernel with Payouts\n  rulec \"../fees/card_fee.rule\"\n"));
    t.write("contexts/payouts.ctx", format!("{payouts}\nshared kernel with Fees\n  rulec \"../fees/card_fee.rule\"\n"));
    let (code, out, _) = ritsu_in(t.path(), &["sakai", "check", "market.ctx"]);
    assert_eq!((code, out.as_str()), (0, "market.ctx: ok — 2 contexts, 2 relationships; 3 artifacts, each in one context; 1 crossing checked (kaname 1)\n"));
    // the book in the other context: a module's `use book` is no crossing, since the host applies
    // the transfers it builds (sakai's DESIGN 18.2)
    std::fs::rename(t.path().join("payouts/ledger.book"), t.path().join("fees/ledger.book")).unwrap();
    t.write("payouts/payout.kn", kn.replace("\"ledger.book\"", "\"../fees/ledger.book\""));
    let (code, out, _) = ritsu_in(t.path(), &["sakai", "check", "market.ctx"]);
    assert_eq!((code, out.as_str()), (0, "market.ctx: ok — 2 contexts, 2 relationships; 3 artifacts, each in one context; 1 crossing checked (kaname 1)\n"));
    // no relationship between the two: E201
    t.write("contexts/payouts.ctx", payouts.split("\nupstream").next().unwrap());
    t.write("contexts/fees.ctx", fees);
    let (code, out, _) = ritsu_in(t.path(), &["sakai", "check", "market.ctx"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.starts_with("error[E201]: payouts/payout.kn:4:1: The file payouts/payout.kn of Payouts refers to fees/card_fee.rule of Fees (use rule), which Payouts has no relationship with\n"), "{out}");
    // yuen: a function of a module, and the module as what verifies the requirement
    let req = "requirements payout v1\ndescription \"What the marketplace owes its sellers for their card payments\"\nrole payments\n\nrequirement fee_from_seller\n  text \"The processor's fee on a payment comes out of the seller's balance\"\n  owner payments\n  decided 2026-10-18 by payments \"As the processor's merchant fees say\"\n  satisfied by kaname \"payouts/payout.kn\" function settle\n  verified by kaname \"payouts/payout.kn\"\n";
    t.write("payout.req", req);
    let (code, out, _) = ritsu_in(t.path(), &["yuen", "check", "payout.req", "--root", "."]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(out.matches("error[E301]").count(), 2, "both links wait for a person to look at them: {out}");
    t.write("payout.req", req.replace("function settle", "function settled"));
    let (_, out, _) = ritsu_in(t.path(), &["yuen", "check", "payout.req", "--root", "."]);
    assert!(out.contains("error[E202]: payout.req:9:16: payouts/payout.kn has no function settled\n"), "{out}");
    assert!(out.contains("  = Did you mean: kaname \"payouts/payout.kn\" function settle\n"), "{out}");
}

/// The pages of the examples as `ritsu kaname doc` draws them (kaname's DESIGN 13), with the
/// verdicts of ritsu's checks across the borders: every function total, and the reasons each
/// transfer can be refused with as chobo finds them at the amounts given. The golden files under
/// `tests/golden/kaname-doc/` hold each English example's page in English and each Japanese one's in
/// Japanese, as Markdown, and the two versions of the payout as HTML. The pages of the rules and the
/// dates files a page holds are rulec's and koyomi's, which kaname's `tests/doc.rs` holds word for
/// word; here each is one line. `RITSU_BLESS=1` writes the golden files again.
#[test]
fn ritsu_kaname_doc_draws_the_examples() {
    let mut wrong = Vec::new();
    for (f, _) in EXAMPLES {
        let at = Path::new(f);
        let dir = kaname_dir().join(at.parent().unwrap());
        let file = at.file_name().unwrap().to_str().unwrap();
        let lang = if file.ends_with(".ja.kn") { "ja" } else { "en" };
        let stem = file.trim_end_matches(".kn");
        let mut forms = vec![("markdown", format!("{stem}.md"))];
        if stem.starts_with("payout") {
            forms.push(("html", format!("{stem}.html")));
        }
        for (format, name) in forms {
            let (code, out, err) = ritsu_in(&dir, &["kaname", "doc", file, "--root", ".", "--format", format, "--lang", lang]);
            assert_eq!(code, 0, "{f}: {out}{err}");
            let page = folded(&out).replace(env!("CARGO_PKG_VERSION"), "<version>");
            if let Err(e) = ritsu_testkit::golden::check(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/kaname-doc").join(&name), &page) {
                wrong.push(e);
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// A page of kaname with the pages of the other languages it holds cut to one line each: in the
/// Markdown, the body of each `<details>`; in the HTML, the JSON that holds them.
fn folded(page: &str) -> String {
    if let Some((head, rest)) = page.split_once("<script type=\"application/json\" id=\"kn-pages\">") {
        let tail = rest.split_once("</script>").map(|(_, t)| t).unwrap_or("");
        return format!("{head}<script type=\"application/json\" id=\"kn-pages\">[the pages]</script>{tail}");
    }
    let mut out = String::new();
    let mut lines = page.lines();
    while let Some(l) = lines.next() {
        out.push_str(l);
        out.push('\n');
        if l == "<details>" {
            if let Some(summary) = lines.next() {
                out.push_str(summary);
                out.push('\n');
            }
            for x in lines.by_ref() {
                if x == "</details>" {
                    break;
                }
            }
            out.push_str("\n[the page]\n\n</details>\n");
        }
    }
    out
}

/// The commands kaname's pages show with what they print (its READMEs, `docs/reference.md` and the
/// skill) print that, run as `ritsu` in kaname's crate: what kaname's own `tests/docs.rs` cannot
/// run, since a module's effects and ritsu's diagnostics across the languages need ritsu-cross. A
/// command shown with no output under it is listed only.
#[test]
fn the_commands_on_kanames_pages_print_what_they_show() {
    let pages = ["README.md", "README.ja.md", "docs/reference.md", "skills/README.md", "../../skills/kaname/SKILL.md", "../../skills/kaname/reference.md"];
    let (mut ran, mut wrong) = (0, Vec::new());
    for page in pages {
        let text = std::fs::read_to_string(kaname_dir().join(page)).unwrap_or_else(|e| panic!("{page}: {e}"));
        let mut block: Option<Vec<String>> = None;
        let mut blocks = Vec::new();
        for line in text.lines() {
            match (line.trim_start().strip_prefix("```"), block.take()) {
                (Some(_), Some(b)) => blocks.push(b),
                (Some(info), None) => block = (info.trim() == "console").then(Vec::new).or(Some(vec!["#not console".to_string()])),
                (None, Some(mut b)) => {
                    b.push(line.to_string());
                    block = Some(b);
                }
                (None, None) => {}
            }
        }
        for b in blocks.into_iter().filter(|b| b.first().is_none_or(|l| l != "#not console")) {
            let mut runs: Vec<(String, Vec<String>)> = Vec::new();
            for l in &b {
                match l.strip_prefix("$ ") {
                    Some(cmd) => runs.push((cmd.to_string(), Vec::new())),
                    None => {
                        if let Some((_, out)) = runs.last_mut() {
                            out.push(l.clone());
                        }
                    }
                }
            }
            for (cmd, out) in runs {
                let Some(rest) = cmd.strip_prefix("ritsu ") else { continue };
                if out.is_empty() {
                    continue;
                }
                let args: Vec<&str> = rest.split_whitespace().collect();
                let (_, got, err) = ritsu_in(&kaname_dir(), &args);
                let got = got + &err;
                let want = out.join("\n") + "\n";
                if got != want {
                    wrong.push(format!("{page}: $ {cmd}\n--- the page shows\n{want}--- it prints\n{got}"));
                }
                ran += 1;
            }
        }
    }
    assert!(ran >= 12, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
