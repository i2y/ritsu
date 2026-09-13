//! Snapshots of the wording (the §13 acceptance condition).
//!
//! Matching codes and matching wording are different things. The wording that §11 decided to
//! "print as is" is pinned together with its rendered result, CJK-width column alignment included.
//!
//! When the wording is changed on purpose, re-bake with `RULEC_BLESS=1 cargo test --test golden`.

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Check `rel`, render only the diagnostics with `code`, and concatenate them.
fn rendered(rel: &str, code: &str) -> String {
    let src = std::fs::read_to_string(root().join(rel)).unwrap_or_else(|_| panic!("読めない: {rel}"));
    let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    let ds = rulec::check_source(&src, rel);
    let hit: Vec<_> = ds.iter().filter(|d| d.code == code).collect();
    assert!(!hit.is_empty(), "{rel} に {code} が出なかった");
    hit.iter()
        .map(|d| rulec::diag::render(d, &lines))
        .collect::<Vec<_>>()
        .join("\n")
}

fn snapshot(name: &str, got: &str) {
    let p = root().join("tests/golden").join(format!("{name}.txt"));
    if std::env::var("RULEC_BLESS").is_ok() || !Path::new(&p).exists() {
        std::fs::write(&p, got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&p).unwrap();
    if want != got {
        panic!(
            "文面が変わりました: {name}\n\
             --- 固定されている文面 ---\n{want}\n\
             --- いまの文面 ---\n{got}\n\
             意図した変更なら RULEC_BLESS=1 cargo test --test golden で焼き直してください。"
        );
    }
}

macro_rules! golden {
    ($fname:ident, $name:literal, $file:literal, $code:literal) => {
        #[test]
        fn $fname() {
            snapshot($name, &rendered($file, $code));
        }
    };
}

// The five that §13 decided to "print as is".
golden!(e101_完全性, "E101", "tests/mutants/m_e101.rule", "E101");
golden!(e101_日付の穴, "E101-date", "tests/mutants/m_e101d.rule", "E101");
golden!(e102_冗長, "E102", "tests/mutants/m_e102.rule", "E102");
golden!(e102_上流の値, "E102-b", "tests/mutants/m_e102b.rule", "E102");
golden!(e103_単位, "E103", "tests/mutants/m_e103.rule", "E103");
golden!(e104_丸め_表引き, "E104-b", "tests/mutants/m_e104.rule", "E104");
golden!(e104_丸め_端数あり, "E104-a", "tests/mutants/m_e104b.rule", "E104");
golden!(e105_重複, "E105", "tests/mutants/m_e105.rule", "E105");

// The rest, for which §11 spells out the wording as well.
golden!(e106_丸めの刻み, "E106", "tests/mutants/m_e106.rule", "E106");
golden!(e107_例, "E107", "tests/mutants/m_e107.rule", "E107");
golden!(e108_溢れ, "E108", "tests/mutants/m_e108.rule", "E108");
golden!(e111_例の出力欠落, "E111", "tests/mutants/m_e111.rule", "E111");
golden!(e112_導出範囲, "E112", "tests/mutants/m_e112.rule", "E112");
golden!(e113_原子, "E113", "tests/mutants/m_e113.rule", "E113");
golden!(w114_未確認の重なり, "W114", "tests/corpus/クーポン併用.rule", "W114");
golden!(w111_未使用, "W111", "tests/mutants/m_w111.rule", "W111");
golden!(w105_要確認, "W105", "tests/mutants/m_w105.rule", "W105");
golden!(w105_負担判定, "W105-b", "tests/corpus/送料.rule", "W105");
golden!(w105_適用順序, "W105-c", "tests/corpus/適用順序.rule", "W105");
golden!(e010_範囲記法, "E010", "tests/mutants/m_e010.rule", "E010");
golden!(e011_別名, "E011", "tests/mutants/m_e011.rule", "E011");
