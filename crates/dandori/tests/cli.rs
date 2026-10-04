//! The dandori binary of this crate holds no rulec (ritsu's DESIGN 2.3): a workflow that uses no rule
//! runs as it always has, and one that uses a rule is told to run with `ritsu dandori`, which reads
//! the rules in the same process. The command is `dandori::cli::run`, whatever port it is handed;
//! the tests hand it rulec's own (ritsu's DESIGN 3.3), the binary one that reads no rule.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The binary, run in the crate's directory.
fn binary(args: &[&str]) -> (u8, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_dandori")).current_dir(root()).args(args).output().expect("could not run dandori");
    (o.status.code().unwrap_or(-1) as u8, String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The command with rulec's port, as `ritsu dandori` runs it, in the crate's directory (where the
/// tests run).
fn with_rulec(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = dandori::cli::run(&args, Rc::new(rulec::ports::Engine::new()), &mut out, &mut err);
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

/// The files under a directory, by their paths from it.
fn written(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.strip_prefix(dir).unwrap().display().to_string(), std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// A flow that uses no rule never asks the port: the binary prints, writes and exits as the command
/// with rulec's port does, for each command.
#[test]
fn a_flow_without_rules_runs_as_it_does_with_rulec() {
    let tmp = TempDir::new("cli");
    let mut compared = 0;
    for flow in ["examples/fulfillment/arrange_delivery.flow", "examples/fulfillment/arrange_delivery.ja.flow", "tests/flows/timeouts.flow", "tests/flows/connect.flow", "tests/flows/service.flow"] {
        assert!(!std::fs::read_to_string(root().join(flow)).unwrap().contains("use rule"), "{flow} uses a rule");
        for (k, args) in [
            vec!["check", flow],
            vec!["check", flow, "--lang", "ja"],
            vec!["check", flow, "--format", "json"],
            vec!["scenarios", flow],
            vec!["doc", flow],
            vec!["doc", flow, "--format", "html", "--lang", "ja"],
            vec!["build", flow, "--target", "temporal", "--out", "@"],
            vec!["build", flow, "--target", "asl", "--out", "@"],
        ]
        .into_iter()
        .enumerate()
        {
            let (a, b) = (tmp.path().join(format!("{compared}-{k}-binary")), tmp.path().join(format!("{compared}-{k}-rulec")));
            let place = |dir: &Path| args.iter().map(|x| if *x == "@" { dir.to_str().unwrap().to_string() } else { x.to_string() }).collect::<Vec<_>>();
            let (pa, pb) = (place(&a), place(&b));
            let (ca, oa, ea) = binary(&pa.iter().map(String::as_str).collect::<Vec<_>>());
            let (cb, ob, eb) = with_rulec(&pb.iter().map(String::as_str).collect::<Vec<_>>());
            let shown = |s: &str, dir: &Path| s.replace(dir.to_str().unwrap(), "@");
            assert_eq!((ca, shown(&oa, &a), shown(&ea, &a)), (cb, shown(&ob, &b), shown(&eb, &b)), "{args:?}");
            assert_eq!(written(&a), written(&b), "{args:?}: what is written");
        }
        compared += 1;
    }
    assert_eq!(compared, 5);
}

/// A flow that uses a rule is told once, at its first `use rule`, that this binary reads no rule
/// and the same command to run through `ritsu dandori` (E018, in either language), with exit 2;
/// nothing that follows from the rules it cannot read is said. With rulec's port the same flow
/// passes (ritsu's DESIGN 2.3).
#[test]
fn a_flow_with_rules_is_told_to_run_with_ritsu_dandori() {
    let flow = "examples/hotel/temporal/hotel.flow";
    let (code, out, err) = binary(&["check", flow]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert_eq!(
        err,
        format!("error[E018]: {flow}:4:10: this dandori cannot read rules, and the flow uses the rule `hold`\n     4 | use rule hold from \"../rules/hold_amount.rule\"\n  = The binary of dandori's own crate holds no other language; run it with every language joined, through ritsu: `ritsu dandori check {flow}`.\n")
    );
    let (code, _, ja) = binary(&["check", flow, "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(ja.contains("  = dandori のクレートのバイナリは、ほかの言語を持ちません。同じコマンドを、すべての言語をつないだ `ritsu dandori check examples/hotel/temporal/hotel.flow --lang ja` のように ritsu で走らせます。"), "{ja}");
    let (code, json, _) = binary(&["build", flow, "--target", "temporal", "--format", "json"]);
    assert_eq!(code, 2, "{json}");
    assert_eq!(with_rulec(&["check", flow]), (0, String::new(), format!("{flow}: ok\n")));
}

/// So is a flow that uses a dates file or a book: once, at the first of them this binary cannot
/// read, with exit 2. rulec's port alone does not read them either; with koyomi's and chobo's
/// joined too, the flow passes.
#[test]
fn a_flow_with_dates_and_books_is_told_to_run_with_ritsu_dandori() {
    let flow = "examples/invoice/invoice.flow";
    let (code, out, err) = binary(&["check", flow]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert_eq!(
        err,
        format!("error[E018]: {flow}:7:11: this dandori cannot read dates files, and the flow uses the dates file `terms`\n     7 | use dates terms from \"dates/payment_terms.cal\"\n  = The binary of dandori's own crate holds no other language; run it with every language joined, through ritsu: `ritsu dandori check {flow}`.\n")
    );
    let (code, _, ja) = binary(&["check", flow, "--lang", "ja"]);
    assert_eq!(code, 2);
    assert!(ja.contains("エラー[E018]: examples/invoice/invoice.flow:7:11: この dandori は日付のファイルを読めません。このフローは日付のファイル `terms` を使います"), "{ja}");
    // with rulec alone, the dates file is still not read
    let (code, _, err) = with_rulec(&["check", flow]);
    assert_eq!(code, 2, "{err}");
    // a book alone
    let dir = ritsu_testkit::TempDir::new("cli-book");
    let book = std::fs::read_to_string(root().join("examples/invoice/books/stock.book")).unwrap();
    std::fs::write(dir.path().join("stock.book"), book).unwrap();
    let only_book = dir.path().join("receive.flow");
    std::fs::write(&only_book, "workflow receive v1\n\nuse book stock from \"stock.book\"\n  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:stock\"\n\ninputs\n  delivery : string\n  sku      : string\n\ntask receive(delivery: string, sku: string, qty: int)\n  book stock.receive.do\n\nflow\n  receive(delivery: delivery, sku: sku, qty: 10)\n").unwrap();
    let (code, _, err) = binary(&["check", only_book.to_str().unwrap()]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains(":3:10: this dandori cannot read books, and the flow uses the book `stock`"), "{err}");
    // with every port joined, both pass
    let joined = |path: &str| {
        let args: Vec<String> = ["check", path].iter().map(|a| a.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = dandori::cli::run_with_ports(&args, Rc::new(rulec::ports::Engine::new()), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), &mut out, &mut err);
        (code, String::from_utf8(err).unwrap())
    };
    assert_eq!(joined(flow), (0, format!("{flow}: ok\n")));
    assert_eq!(joined(only_book.to_str().unwrap()).0, 0);
}

/// `cli::run` takes the words after the program's name and writes where it is told: a page asked
/// for to `out`, what is wrong to `err`, and answers the exit code.
#[test]
fn the_command_writes_where_it_is_told() {
    let (code, out, err) = with_rulec(&["--version"]);
    assert_eq!((code, out, err), (0, format!("dandori {}\n", env!("CARGO_PKG_VERSION")), String::new()));
    let (code, out, err) = with_rulec(&["check", "--help"]);
    assert!(code == 0 && out.starts_with("dandori check") && err.is_empty(), "{code} {out} {err}");
    let (code, out, err) = with_rulec(&["nope"]);
    assert_eq!((code, out.as_str(), err.as_str()), (2, "", "error: there is no command `nope`; run `dandori --help`\n"));
    let (code, out, err) = with_rulec(&[]);
    assert!(code == 2 && out.is_empty() && err.contains("Usage"), "{code} {out} {err}");
}
