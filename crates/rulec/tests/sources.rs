//! Sources (§15.68): a `source` declared, cited with `@`, and held to the
//! copies beside the rule by pinned digests. Fetching is not exercised here — it reads the
//! network — but everything from a copy on disk onward is.

use std::path::{Path, PathBuf};
use std::process::Command;

const RULE: &str = "tests/corpus/印紙税の本則と軽減.rule";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A scratch directory holding the corpus rule and the copies of its sources.
fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-sources-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    copy_dir(&root().join("tests/corpus/sources"), &d.join("sources"));
    std::fs::copy(root().join(RULE), d.join("a.rule")).unwrap();
    d
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

fn codes(src: &str, path: &Path) -> Vec<String> {
    rulec::check_source(src, &path.to_string_lossy()).iter().map(|d| d.code.to_string()).collect()
}

fn rulec(dir: &Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr))
}

#[test]
fn 写しと固定が合っていれば通る() {
    let src = std::fs::read_to_string(root().join(RULE)).unwrap();
    let cs = codes(&src, &root().join(RULE));
    assert!(cs.iter().all(|c| c.starts_with('W')) && !cs.contains(&"W119".to_string()), "{cs:?}");
}

#[test]
fn 固定の欠け_写しの変化_写しの欠け_余った固定() {
    let d = scratch("diags");
    let src = std::fs::read_to_string(d.join("a.rule")).unwrap();
    let pin = "  第91条 sha256:85faf53f6f6e8196\n";
    assert!(src.contains(pin), "the corpus rule pins 第91条");

    let no_pin = src.replace(pin, "");
    assert!(codes(&no_pin, &d.join("a.rule")).contains(&"E037".to_string()));

    let bad = src.replace("sha256:85faf53f6f6e8196", "sha256:0000000000000000");
    let ds = rulec::check_source(&bad, &d.join("a.rule").to_string_lossy());
    let e = ds.iter().find(|d| d.code == "E038").expect("E038");
    assert!(e.notes.iter().any(|n| n.contains("表 軽減") && n.contains("定義 軽減期間")), "names the citing definitions: {:?}", e.notes);
    assert_eq!(e.fix.text.as_deref(), Some(pin.trim_end()), "the fix is the pin line");

    // A copy refreshed to different content: the pin no longer matches.
    let p = d.join("sources/law/332AC0000000026@2026-04-01/MainProvision-Article_91.xml");
    let xml = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, xml.replace("第九十一条", "第九十一条の二")).unwrap();
    assert!(codes(&src, &d.join("a.rule")).contains(&"E038".to_string()));
    std::fs::write(&p, &xml).unwrap();

    let no_copy = src.replace("table 軽減(reduced_rate)  @措置法 第91条", "table 軽減(reduced_rate)  @措置法 第92条");
    assert!(codes(&no_copy, &d.join("a.rule")).contains(&"E039".to_string()));

    let extra = src.replace(pin, &format!("{pin}  第92条 sha256:0000000000000000\n"));
    assert!(codes(&extra, &d.join("a.rule")).contains(&"W119".to_string()));

    let undeclared = src.replace("@措置法 第91条", "@措置 第91条");
    assert!(codes(&undeclared, &d.join("a.rule")).contains(&"E012".to_string()));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn pinは固定行だけを書き換える() {
    let d = scratch("pin");
    let src = std::fs::read_to_string(d.join("a.rule")).unwrap();
    let pin = "  第91条 sha256:85faf53f6f6e8196\n";
    // Drop one pin, add a stale one: `pin` restores the first and removes the second.
    let broken = src.replace(pin, "  第92条 sha256:0000000000000000\n");
    std::fs::write(d.join("a.rule"), &broken).unwrap();
    let (c, out) = rulec(&d, &["source", "pin", "a.rule", "--lang", "ja"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("固定しました"), "{out}");
    let after = std::fs::read_to_string(d.join("a.rule")).unwrap();
    assert_eq!(after, src, "everything but the pin lines is untouched, and the pins are the copies' digests");
    let (c, out) = rulec(&d, &["check", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fmtは引用と固定行を整える() {
    let src = "\
rule t(t) v1

source 法 = law \"000AC0000000001\" asof 2026-04-01
   第1条   sha256:ce31217424a10206
 第20条の2 sha256:0000000000000000

inputs
  a(a) : bool

outputs
  x(x) : bool

table 表(t1)   @法 第1条
| a | -> x |
| - | true |   @法 第20条の2   # 注記
";
    let once = rulec::fmt::format(src);
    assert!(once.contains("source 法 = law \"000AC0000000001\" asof 2026-04-01\n  第1条     sha256:ce31217424a10206\n  第20条の2 sha256:0000000000000000\n"), "{once}");
    assert!(once.contains("| - | true |  @法 第20条の2  # 注記\n"), "{once}");
    assert_eq!(rulec::fmt::format(&once), once, "not idempotent");
}

#[test]
fn ページは引いた断片を引用する() {
    let (c, out) = rulec(&root(), &["doc", RULE, "--lang", "ja"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("出典: 措置法 第91条（法令 332AC0000000026、2026-04-01 時点）"), "{out}");
    assert!(out.contains("> 第九十一条"), "the article's title is quoted");
    assert!(out.contains("| 軽減期間 | 定義 | `作成日 <= 2027-03-31` |  | 出典: 措置法 第91条 |"), "the define's citation is a note, not part of its expression");
}

/// A file beside the rule is cited whole (`@郵便`) or with a word saying where in it; a law
/// is copied an article at a time, so a citation of a law without one is E037.
#[test]
fn ファイルは丸ごと引用でき_法令は箇所が要る() {
    let d = scratch("whole");
    std::fs::write(d.join("料金表.txt"), "S60 990円\n").unwrap();
    let h = rulec::sha256::short(b"S60 990\xe5\x86\x86\n");
    let head = format!(
        "rule t(t) v1\n\nsource 郵便 = file \"料金表.txt\" sha256:{h}\nsource 措置法 = law \"332AC0000000026\" asof 2026-04-01\n  第91条 sha256:85faf53f6f6e8196\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n"
    );
    let whole = format!("{head}table 表(t1)  @郵便\n| a | -> x |\n| - | true |\n\ntable 表2(t2)  @郵便 別紙1\n| a | -> y(y) : bool |\n| - | true |\n\ntable 表3(t3)  @措置法 第91条\n| a | -> z(z) : bool |\n| - | true |\n");
    let p = d.join("a.rule");
    assert!(codes(&whole, &p).iter().all(|c| c.starts_with('W')), "{:?}", codes(&whole, &p));
    let bare_law = format!("{head}table 表(t1)  @措置法\n| a | -> x |\n| - | true |\n");
    assert!(codes(&bare_law, &p).contains(&"E037".to_string()), "{:?}", codes(&bare_law, &p));
    let _ = std::fs::remove_dir_all(&d);
}
