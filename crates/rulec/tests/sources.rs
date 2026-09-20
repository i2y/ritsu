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
    assert!(out.contains("出典: 措置法 第91条（法令 332AC0000000026、2026-04-01 時点。2026-04-01 施行、令和8年法律第12号による改正後）"), "{out}");
    assert!(out.contains("> 第九十一条"), "the article's title is quoted");
    assert!(out.contains("| 軽減期間 | 定義 | `作成日 <= 2027-03-31` |  | 出典: 措置法 第91条 |"), "the define's citation is a note, not part of its expression");
}

/// A file beside the rule is cited whole (`@郵便`); a law is copied an article at a time, so a
/// citation of a law without one is E037. A document's fragments are its tables, so a word
/// that is not one (`別紙1`) is E037 too (§15.82).
#[test]
fn ファイルは丸ごと引用でき_法令は箇所が要る() {
    let d = scratch("whole");
    std::fs::write(d.join("料金表.txt"), "S60 990円\n").unwrap();
    let h = rulec::sha256::short(b"S60 990\xe5\x86\x86\n");
    let head = format!(
        "rule t(t) v1\n\nsource 郵便 = file \"料金表.txt\" sha256:{h}\nsource 措置法 = law \"332AC0000000026\" asof 2026-04-01\n  第91条 sha256:85faf53f6f6e8196\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n"
    );
    let whole = format!("{head}table 表(t1)  @郵便\n| a | -> x |\n| - | true |\n\ntable 表3(t3)  @措置法 第91条\n| a | -> z(z) : bool |\n| - | true |\n");
    let p = d.join("a.rule");
    assert!(codes(&whole, &p).iter().all(|c| c.starts_with('W')), "{:?}", codes(&whole, &p));
    let bare_law = format!("{head}table 表(t1)  @措置法\n| a | -> x |\n| - | true |\n");
    assert!(codes(&bare_law, &p).contains(&"E037".to_string()), "{:?}", codes(&bare_law, &p));
    let odd = format!("{head}table 表(t1)  @郵便 別紙1\n| a | -> x |\n| - | true |\n");
    let ds = rulec::check_source(&odd, &p.to_string_lossy());
    let e = ds.iter().find(|x| x.code == "E037").expect("E037");
    assert!(e.notes.iter().any(|n| n.contains("表3")), "the note says how a document's fragment is written: {:?}", e.notes);
    let _ = std::fs::remove_dir_all(&d);
}

/// A document's tables are fragments, held to their copies exactly as a law's articles are
/// (§15.82): `fetch` takes them out of the document, `pin` writes their digests, `check`
/// reads the copies and never the document, and `doc` quotes the table under the rows.
#[test]
fn 文書の表を引いて写しに固定する() {
    let d = scratch("fragments");
    let doc = "# 料金表\n\n前書き。\n\n| あて先 | S60 |\n|---|---|\n| 近畿 | 990円 |\n| 関東 | 880円 |\n";
    std::fs::write(d.join("料金表.md"), doc).unwrap();
    let rule = "rule t(t) v1\n\nsource 料金表 = file \"料金表.md\"\n\nenum あて先(dest) = 近畿(kinki) | 関東(kanto)\n\ninputs\n  あて先(dest) : あて先\n\noutputs\n  運賃(fee) : money[円]  round up(10円)\n\ntable 運賃表(fee_table)  @料金表 表1\n| あて先 | -> 運賃 |\n| 近畿   | 990円   |\n| 関東   | 880円   |\n";
    let p = d.join("a.rule");
    std::fs::write(&p, rule).unwrap();

    // No copy yet: the digest of the document is not pinned (E037) and the table has not been
    // taken out of it (E039). `check` says where to look; it does not read the document.
    let cs = codes(rule, &p);
    assert!(cs.contains(&"E037".to_string()) && cs.contains(&"E039".to_string()), "{cs:?}");

    let (c, out) = rulec(&d, &["source", "fetch", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("表1") && out.contains("3") && out.contains("2"), "the report says the size of the table: {out}");
    let tsv = std::fs::read_to_string(d.join("料金表.md.fragments/表1.tsv")).unwrap();
    assert_eq!(tsv, "あて先\tS60\n近畿\t990円\n関東\t880円\n", "the copy is the table, one row per line");

    let (c, out) = rulec(&d, &["source", "pin", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let after = std::fs::read_to_string(&p).unwrap();
    let pin = format!("  表1 sha256:{}", rulec::sha256::short(tsv.as_bytes()));
    assert!(after.contains(&pin), "the fragment is pinned under the source line:\n{after}");
    assert!(rulec::check_source(&after, &p.to_string_lossy()).iter().all(|x| x.code.starts_with('W')), "{after}");

    // The table under the rows, for the approver.
    let (c, out) = rulec(&d, &["doc", "a.rule", "--lang", "ja"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("出典: 料金表 表1（料金表.md"), "{out}");
    assert!(out.contains("> | 近畿 | 990円 |"), "the fragment is quoted as the table it is:\n{out}");

    // A price moves in the document: both the document and the table it holds are reported,
    // and the row to reread is named.
    std::fs::write(d.join("料金表.md"), doc.replace("880円", "900円")).unwrap();
    let (c, _) = rulec(&d, &["source", "fetch", "a.rule"]);
    assert_eq!(c, 0);
    let ds = rulec::check_source(&after, &p.to_string_lossy());
    let moved: Vec<&str> = ds.iter().filter(|x| x.code == "E038").map(|x| x.title.as_str()).collect();
    assert_eq!(moved.len(), 2, "the document and the fragment both moved: {ds:?}");
    assert!(ds.iter().any(|x| x.code == "E038" && x.notes.iter().any(|n| n.contains("運賃表"))), "{ds:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Every generated file names the documents the rule transcribes, in the same words in every
/// language (§15.71).
#[test]
fn 生成物のヘッダは出典を名指す() {
    let d = std::env::temp_dir().join(format!("rulec-cites-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let (c, out) = rulec(&root(), &["gen", RULE, "--out", &d.to_string_lossy(), "--lang", "en"]);
    assert_eq!(c, 0, "{out}");
    let py = std::fs::read_to_string(d.join("python/stamp_duty_split.py")).unwrap();
    assert!(py.contains("# Cites: 法 = law 342AC0000000023 asof 2026-04-01 (別表第一 sha256:0ba69792e960021e)\n"), "{py}");
    assert!(py.contains("# Cites: 措置法 = law 332AC0000000026 asof 2026-04-01 (第91条 sha256:85faf53f6f6e8196)\n"), "{py}");
    let sql = std::fs::read_to_string(d.join("sql/stamp_duty_split.sql")).unwrap();
    assert!(sql.contains("-- Cites: 措置法 = law 332AC0000000026 asof 2026-04-01 (第91条 sha256:85faf53f6f6e8196)\n"), "{sql}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A file source may say where its copy came from (§15.76). The address rides through
/// everything that shows a source: the fix for an unpinned digest, `pin`, the generated
/// header and `rulec api` — otherwise a reader of the generated code could not go and look at
/// the document the rows were transcribed from.
#[test]
fn ファイルの出典はurlを持ち_それが下流まで届く() {
    let d = scratch("url");
    std::fs::write(d.join("料金表.txt"), "S60 990円\n").unwrap();
    let url = "https://raw.githubusercontent.com/o/r/a1b2c3d4e5f60718293a4b5c6d7e8f9012345678/docs/t.md";
    let body = |pin: &str| {
        format!(
            "rule t(t) v1\n\nsource 郵便 = file \"料金表.txt\" url \"{url}\"{pin}\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\ntable 表(t1)  @郵便\n| a | -> x |\n| - | true |\n"
        )
    };
    let p = d.join("a.rule");

    // Unpinned: E037, and the fix keeps the address rather than dropping it.
    let unpinned = body("");
    let ds = rulec::check_source(&unpinned, &p.to_string_lossy());
    let e037 = ds.iter().find(|x| x.code == "E037").expect("E037 が出ない");
    let fix = e037.fix.text.as_deref().expect("fix の本文が無い");
    assert!(fix.contains(&format!("url \"{url}\"")), "{fix}");

    // `pin` writes the digest without disturbing the address.
    std::fs::write(&p, &unpinned).unwrap();
    let (c, out) = rulec(&d, &["source", "pin", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let after = std::fs::read_to_string(&p).unwrap();
    let line = after.lines().find(|l| l.starts_with("source 郵便")).unwrap();
    assert!(line.contains(&format!("url \"{url}\"")) && line.contains("sha256:"), "{line}");
    assert!(rulec::check_source(&after, &p.to_string_lossy()).iter().all(|x| x.code.starts_with('W')));

    // The header and the inventory carry it.
    let g = d.join("gen");
    let (c, out) = rulec(&d, &["gen", "a.rule", "--out", &g.to_string_lossy(), "--lang", "en"]);
    assert_eq!(c, 0, "{out}");
    let py = std::fs::read_to_string(g.join("python/t.py")).unwrap();
    assert!(py.contains(&format!("# Cites: 郵便 = file 料金表.txt url {url} sha256:")), "{py}");
    let (c, api) = rulec(&d, &["api", "a.rule"]);
    assert_eq!(c, 0, "{api}");
    assert!(api.contains(&format!("\"url\":\"{url}\"")), "{api}");

    // An address is optional, and a word that is not one is refused.
    let none = body("").replace(&format!(" url \"{url}\""), "");
    assert!(rulec::check_source(&none, &p.to_string_lossy()).iter().any(|x| x.code == "E037"));
    let bad = body("").replace(&format!("url \"{url}\""), "urrl \"x\"");
    assert!(rulec::check_source(&bad, &p.to_string_lossy()).iter().any(|x| x.code.starts_with('E')));
    let _ = std::fs::remove_dir_all(&d);
}

/// The rows against the copy they say they were transcribed from (§15.82): an amount that the
/// copy does not show is E116, a number the copy states that no row uses is W120, and the two
/// together are what a mistyped digit looks like. What the table rewrote on the way in — a
/// range over rows the copy lists one by one — is neither.
#[test]
fn 写した金額と写しを突き合わせる() {
    let d = scratch("transcribe");
    let doc = "# 料金表\n\n| あて先 | S60 | S80 |\n|---|---|---|\n| 近畿 | 990円 | 1210円 |\n| 関東 | 880円 | 1100円 |\n";
    std::fs::write(d.join("料金表.md"), doc).unwrap();
    let rule = |fee: &str, cite_table: &str, cite_row: &str| {
        format!(
            "rule t(t) v1\n\nsource 料金表 = file \"料金表.md\"\n\nenum あて先(dest) = 近畿(kinki) | 関東(kanto)\nenum 区分(size) = S60(s60) | S80(s80)\n\n\
             inputs\n  あて先(dest) : あて先\n  サイズ(size) : 区分\n\noutputs\n  運賃(fee) : money[円]  round up(10円)\n\n\
             table 運賃表(fee_table){cite_table}\npolicy unique\n| あて先 | サイズ | -> 運賃 |\n\
             | 近畿 | S60 | 990円 |{cite_row}\n| 近畿 | S80 | 1210円 |{cite_row}\n| 関東 | S60 | {fee} |{cite_row}\n| 関東 | S80 | 1100円 |{cite_row}\n"
        )
    };
    let p = d.join("a.rule");
    std::fs::write(&p, rule("880円", "  @料金表 表1", "")).unwrap();
    let (c, out) = rulec(&d, &["source", "fetch", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec(&d, &["source", "pin", "a.rule"]);
    assert_eq!(c, 0, "{out}");
    let pinned = std::fs::read_to_string(&p).unwrap();
    let pins: String = pinned.lines().take(5).filter(|l| l.contains("sha256")).collect::<Vec<_>>().join("\n");

    // Transcribed as the copy has it: nothing to say.
    assert!(codes(&pinned, &p).is_empty(), "{:?}", codes(&pinned, &p));

    // One digit wrong. The amount is on the rounding grid, complete and unique — every other
    // check is green, and these two are not.
    let typo = pinned.replace("| 880円 |", "| 890円 |");
    let ds = rulec::check_source(&typo, &p.to_string_lossy());
    let e = ds.iter().find(|x| x.code == "E116").expect("E116");
    assert!(e.title.contains("行3"), "{}", e.title);
    assert!(e.marks.iter().any(|m| m.label.contains("890円")), "the value is on the mark: {:?}", e.marks);
    let w = ds.iter().find(|x| x.code == "W120").expect("W120");
    assert!(w.notes.iter().any(|n| n.contains("880円")), "the value left unused is named: {:?}", w.notes);

    // The citation on the rows says only where each row came from, so the rest of the copy
    // goes unasked — and the amounts are still held to it.
    let by_row = rule("890円", "", "  @料金表 表1").replace("source 料金表 = file \"料金表.md\"", &pins);
    let ds = rulec::check_source(&by_row, &p.to_string_lossy());
    assert!(ds.iter().any(|x| x.code == "E116"), "{ds:?}");
    assert!(!ds.iter().any(|x| x.code == "W120"), "{ds:?}");

    // A table that merges what the copy lists one by one has transcribed all of it.
    std::fs::write(d.join("重量.md"), "| 重量 | 追加 |\n|---|---|\n| 1kg | 100円 |\n| 2kg | 100円 |\n| 3kg | 100円 |\n| 4kg | 250円 |\n").unwrap();
    let merged = "rule t(t) v1\n\nsource 表 = file \"重量.md\"\n\ninputs\n  重量(w) : mass[kg]  range >=1kg <=4kg\n\noutputs\n  追加(extra) : money[円]  round up(10円)\n\ntable 追加表(extra_table)  @表 表1\npolicy first\n| 重量 | -> 追加 |\n| <=3kg | 100円 |\n| - | 250円 |\n";
    let q = d.join("b.rule");
    std::fs::write(&q, merged).unwrap();
    let (c, out) = rulec(&d, &["source", "fetch", "b.rule"]);
    assert_eq!(c, 0, "{out}");
    let (c, out) = rulec(&d, &["source", "pin", "b.rule"]);
    assert_eq!(c, 0, "{out}");
    let merged = std::fs::read_to_string(&q).unwrap();
    let cs = codes(&merged, &q);
    assert!(!cs.contains(&"W120".to_string()) && !cs.contains(&"E116".to_string()), "{cs:?}");
    let _ = std::fs::remove_dir_all(&d);
}
