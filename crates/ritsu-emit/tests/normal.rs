//! NFC and NFKC (src/normal.rs) as Python's `unicodedata` writes them: each character it has
//! assigned, the decomposed form of each that decomposes, and marks in both orders after a letter,
//! which is what reordering and a blocked composition come to. The tables are of Unicode 17.0, and
//! Python's may be older: what it has not assigned is not compared. chobo's DESIGN 18 says how the
//! module was held to the conformance file of the Unicode Character Database.

use ritsu_emit::normal::{nfc, nfkc};
use ritsu_testkit::level::{Need, ready};
use ritsu_testkit::skip::skip;
use ritsu_testkit::tmp::TempDir;
use std::process::Command;

const COMPARE: &str = r#"import sys, unicodedata as u
out = open(sys.argv[1], "w", encoding="utf-8")
print(u.unidata_version, file=out)
def line(s):
    hex = lambda x: " ".join(f"{ord(c):X}" for c in x)
    print(hex(s), hex(u.normalize("NFC", s)), hex(u.normalize("NFKC", s)), sep=";", file=out)
MARKS = [chr(0x301), chr(0x323), chr(0x307), chr(0x345), chr(0x5B0), chr(0x3099)]
for code in range(0x110000):
    if 0xD800 <= code < 0xE000:
        continue
    c = chr(code)
    if u.category(c) == "Cn":
        continue
    line(c)
    d = u.normalize("NFD", c)
    if d != c:
        line(d)
        line(d + MARKS[code % len(MARKS)])
for base in "aAeEoOuUsSkKwzα":
    for x in MARKS:
        for y in MARKS:
            line(base + x + y)
# the syllables of Hangul as letters, with a final that makes none
for s in [0x1100, 0x1112]:
    for v in [0x1161, 0x1175]:
        for t in [0x11A7, 0x11A8, 0x11C2, 0x11C3]:
            line(chr(s) + chr(v) + chr(t))
out.close()
"#;

#[test]
fn the_forms_are_what_python_writes() {
    let py = "python3";
    if !ready(Need::Python, || ritsu_testkit::tools::runs(py, &["--version"]), "no python3; NFC and NFKC are not held to unicodedata") {
        return;
    }
    let work = TempDir::new("normal");
    let (script, file) = (work.path().join("compare.py"), work.path().join("forms.txt"));
    std::fs::write(&script, COMPARE).unwrap();
    let out = Command::new(py).arg("-I").arg(&script).arg(&file).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&file).unwrap();
    let mut lines = text.lines();
    let version: Vec<u8> = lines.next().unwrap().split('.').map(|v| v.parse().unwrap()).collect();
    let (a, b, c) = ritsu_emit::normal_table::UNICODE;
    if (version[0], version[1], version[2]) > (a, b, c) {
        skip(&format!("Python's Unicode {version:?} is newer than the tables' {:?}; NFC and NFKC are not held to it", (a, b, c)));
        return;
    }
    let chars = |hex: &str| -> String { hex.split(' ').map(|x| char::from_u32(u32::from_str_radix(x, 16).unwrap()).unwrap()).collect() };
    let (mut compared, mut wrong) = (0usize, Vec::new());
    for l in lines {
        let mut f = l.split(';');
        let (s, c, k) = (chars(f.next().unwrap()), chars(f.next().unwrap()), chars(f.next().unwrap()));
        compared += 1;
        if nfc(&s) != c || nfkc(&s) != k {
            wrong.push(l.to_string());
        }
    }
    assert!(wrong.is_empty(), "{} of {compared} differ:\n{}", wrong.len(), wrong.iter().take(20).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("compared: NFC and NFKC of {compared} strings with Python's unicodedata (Unicode {version:?}; the tables' is {:?})", (a, b, c));
}

/// What the notes of W108 rest on, without Python.
#[test]
fn the_forms_of_the_names_of_the_notes() {
    assert_eq!(nfc("shop"), "shop");
    assert_eq!(nfkc("ｓｈｏｐ"), "shop");
    assert_eq!(nfc("ｓｈｏｐ"), "ｓｈｏｐ");
    assert_eq!(nfkc("ﾛｯﾄ"), "ロット");
    assert_eq!(nfc(&String::from(char::from_u32(0x212A).unwrap())), "K");
    let compat = String::from(char::from_u32(0xFA19).unwrap());
    assert_eq!(nfc(&compat), String::from(char::from_u32(0x795E).unwrap()));
    let letters: String = [0x1112, 0x1161, 0x11AB].iter().filter_map(|u| char::from_u32(*u)).collect();
    assert_eq!(nfc(&letters), String::from(char::from_u32(0xD55C).unwrap()));
}
