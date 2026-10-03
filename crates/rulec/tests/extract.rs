//! The extraction adapter (§15.82, fourth stage): `rulec source fetch --via <cmd>`.
//!
//! A PDF or a scan needs an extractor that is not this program. What is held here is the
//! protocol rather than any particular extractor: a fake one, written by the test, stands in
//! for docling — which is the point of the shape, since whatever reads the document, what is
//! checked afterwards is the copy that was pinned.

use std::path::PathBuf;
use std::process::Command;

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok()
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-extract-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rulec(d: &PathBuf, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(d).args(args).output().expect("rulec を起動できない");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr),
    )
}

/// Write an extractor that says `body` and make it runnable.
fn extractor(d: &PathBuf, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, format!("#!/usr/bin/env python3\nimport json, sys\n{body}")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    format!("./{name}")
}

const RULE: &str = "rule t(t) v1\n\nsource 料金表 = file \"料金表.pdf\"\n\nenum あて先(dest) = 近畿(kinki) | 関東(kanto)\n\n\
                    inputs\n  あて先(dest) : あて先\n\noutputs\n  運賃(fee) : money[円]  round up(10円)\n\n\
                    table 運賃表(fee_table)  @料金表 表1\npolicy unique\n| あて先 | -> 運賃 |\n| 近畿 | 990円 |\n| 関東 | 880円 |\n";

const GOOD: &str = "print(json.dumps({'rulec': 'extract/1', 'impl': 'fake-extractor 0.1'}), flush=True)\n\
                    print(json.dumps({'block': 'heading', 'text': '料金表'}, ensure_ascii=False), flush=True)\n\
                    print(json.dumps({'block': 'table', 'page': 12, 'grid': [['あて先','S60'],['近畿','990円'],['関東','880円']]}, ensure_ascii=False), flush=True)\n\
                    print(json.dumps({'done': True}), flush=True)\n";

fn scratch(tag: &str) -> PathBuf {
    let d = dir(tag);
    std::fs::write(d.join("料金表.pdf"), b"not really a pdf\n").unwrap();
    std::fs::write(d.join("a.rule"), RULE).unwrap();
    d
}

/// Without an extractor a PDF is refused by name; with one the table comes out of it, the
/// copy is pinned, and everything downstream — including the amounts check — works as it does
/// for a format this program reads itself.
#[test]
fn 抽出器を通して写しを取る() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = scratch("via");
    let (c, out) = rulec(&d, &["source", "fetch", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains(".pdf") && out.contains("csv"), "the formats it does read are named: {out}");

    let cmd = extractor(&d, "extract.py", GOOD);
    let (c, out) = rulec(&d, &["source", "fetch", "a.rule", "--via", &cmd]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("12"), "the page the extractor gave is in the report: {out}");
    assert!(out.contains("fake-extractor 0.1"), "{out}");
    assert_eq!(
        std::fs::read_to_string(d.join("料金表.pdf.fragments/表1.tsv")).unwrap(),
        "あて先\tS60\n近畿\t990円\n関東\t880円\n"
    );
    // Who read the document stays beside the copy, and the approver's page says it.
    assert_eq!(std::fs::read_to_string(d.join("料金表.pdf.fragments/extractor.txt")).unwrap(), "fake-extractor 0.1\n");

    let (c, out) = rulec(&d, &["source", "pin", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec(&d, &["check", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec(&d, &["doc", "a.rule", "--lang", "ja"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("fake-extractor 0.1 が読んだ"), "{out}");
    assert!(out.contains("> | 近畿 | 990円 |"), "{out}");

    // The amounts are held to what the extractor produced, like any other copy.
    let typo = std::fs::read_to_string(d.join("a.rule")).unwrap().replace("| 近畿 | 990円 |", "| 近畿 | 890円 |");
    std::fs::write(d.join("a.rule"), typo).unwrap();
    let (c, out) = rulec(&d, &["check", "a.rule"]);
    assert_eq!(c, 1, "{out}");
    assert!(out.contains("E116"), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// An extractor that does not name itself, one that stops half way, and one that fails: all
/// three stop the command rather than leaving a copy that looks like a reading of the
/// document. A half-read stream is the dangerous one — `表3` would quietly be another table.
#[test]
fn 途中で終わった抽出は写しにしない() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = scratch("bad");
    let good = extractor(&d, "good.py", GOOD);
    let (c, _) = rulec(&d, &["source", "fetch", "a.rule", "--via", &good]);
    assert_eq!(c, 0);
    let before = std::fs::read_to_string(d.join("料金表.pdf.fragments/表1.tsv")).unwrap();

    let cases = [
        ("nameless.py", "print(json.dumps({'block': 'table', 'grid': [['a']]}), flush=True)\nprint(json.dumps({'done': True}), flush=True)\n", "extract/1"),
        (
            "half.py",
            "print(json.dumps({'rulec': 'extract/1', 'impl': 'half 0.1'}), flush=True)\nprint(json.dumps({'block': 'table', 'grid': [['a']]}), flush=True)\n",
            "done",
        ),
        ("crashy.py", "print(json.dumps({'rulec': 'extract/1', 'impl': 'crashy 0.1'}), flush=True)\nsys.exit(3)\n", "3"),
    ];
    for (name, body, says) in cases {
        let cmd = extractor(&d, name, body);
        let (c, out) = rulec(&d, &["source", "fetch", "a.rule", "--via", &cmd]);
        assert_eq!(c, 2, "{name}: {out}");
        assert!(out.contains(says), "{name} says what went wrong: {out}");
        assert!(out.contains("料金表"), "{name} names the source: {out}");
        // The copy that was approved is still the copy.
        assert_eq!(std::fs::read_to_string(d.join("料金表.pdf.fragments/表1.tsv")).unwrap(), before, "{name}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The template is the shape of an extractor, and it says which documents of this rule need
/// one at all — a question only the rule can answer.
#[test]
fn テンプレートは文書を名指しする() {
    let d = scratch("template");
    let (c, out) = rulec(&d, &["source", "fetch", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec(&d, &["adapter", "a.rule", "--template", "docling", "--lang", "ja"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("料金表.pdf"), "the document that needs one is named: {out}");
    assert!(out.contains("extract/1") && out.contains("\"done\": True"), "{out}");
    assert!(out.contains("rulec source fetch a.rule --via"), "the line that runs it names this rule: {out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The template, run the way its own usage line says — `--via ./extract.py`, so the `#!` line
/// has to come first or a shell reads the file — against a stand-in for docling with the same
/// calls: `export_to_dataframe` wants the document, and a table docling found no header row
/// in names its columns 0, 1, 2, …, a row the document does not have.
#[test]
fn テンプレートは書いてある使い方のとおりに動く() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let d = scratch("docling");
    std::fs::write(d.join("a.rule"), RULE.replace("@料金表 表1", "@料金表 表1, 表2")).unwrap();
    let (c, out) = rulec(&d, &["adapter", "a.rule", "--template", "docling"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.starts_with("#!/usr/bin/env python3\n"), "the `#!` line is not the first:\n{out}");
    let p = d.join("extract.py");
    std::fs::write(&p, &out).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::create_dir_all(d.join("docling")).unwrap();
    std::fs::write(d.join("docling/__init__.py"), "__version__ = \"0-stub\"\n").unwrap();
    std::fs::write(
        d.join("docling/document_converter.py"),
        "class _Values:\n    def __init__(self, rows): self.rows = rows\n    def tolist(self): return self.rows\n\n\
         class _Frame:\n    def __init__(self, columns, rows): self.columns, self.values = columns, _Values(rows)\n\n\
         class _Prov:\n    page_no = 12\n\n\
         class _Table:\n    prov = [_Prov()]\n    def __init__(self, columns, rows): self.frame = _Frame(columns, rows)\n    \
         def export_to_dataframe(self, doc=None):\n        assert doc is not None, 'export_to_dataframe wants the document'\n        return self.frame\n\n\
         class _Doc:\n    tables = [_Table(['あて先', 'S60'], [['近畿', '990円'], ['関東', '880円']]), _Table([0, 1], [['注', '税込']])]\n\n\
         class _Result:\n    document = _Doc()\n\n\
         class DocumentConverter:\n    def convert(self, path): return _Result()\n",
    )
    .unwrap();
    let (c, out) = rulec(&d, &["source", "fetch", "a.rule", "--via", "./extract.py"]);
    assert_eq!(c, 0, "{out}");
    let copies = d.join("料金表.pdf.fragments");
    assert_eq!(std::fs::read_to_string(copies.join("表1.tsv")).unwrap(), "あて先\tS60\n近畿\t990円\n関東\t880円\n");
    assert_eq!(std::fs::read_to_string(copies.join("表2.tsv")).unwrap(), "注\t税込\n", "the numbered header was written as a row");
    assert_eq!(std::fs::read_to_string(copies.join("extractor.txt")).unwrap(), "docling 0-stub\n");
    let _ = std::fs::remove_dir_all(&d);
}
