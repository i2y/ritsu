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
    rulec::i18n::set(rulec::i18n::Lang::Ja);
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
golden!(e101_端数の刻み, "E101-frac", "tests/mutants/m_e101fracja.rule", "E101");
golden!(e101_定義の列, "E101-ratio", "tests/mutants/m_e101ratioja.rule", "E101");
golden!(e102_冗長, "E102", "tests/mutants/m_e102.rule", "E102");
golden!(e102_上流の値, "E102-b", "tests/mutants/m_e102b.rule", "E102");
golden!(e102_上流の相関, "E102-c", "tests/mutants/m_e102c.rule", "E102");
golden!(e103_単位, "E103", "tests/mutants/m_e103.rule", "E103");
golden!(e103_単位の無い小数, "E103-bare", "tests/mutants/m_e103bareja.rule", "E103");
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
golden!(w114_未確認の重なり, "W114", "tests/mutants/m_w114.rule", "W114");
golden!(w111_未使用, "W111", "tests/mutants/m_w111.rule", "W111");
golden!(w105_要確認, "W105", "tests/mutants/m_w105.rule", "W105");
golden!(w105_負担判定, "W105-b", "tests/corpus/送料.rule", "W105");
golden!(w105_適用順序, "W105-c", "tests/corpus/適用順序.rule", "W105");
// The same two over their twins (tests/corpus/twins.tsv): the same findings, worded the same way,
// with English names in the source lines the frames quote.
golden!(w105_負担判定_twin, "W105-b-twin", "tests/corpus/member_shipping_fee.rule", "W105");
golden!(w105_適用順序_twin, "W105-c-twin", "tests/corpus/coupon_order.rule", "W105");
golden!(e010_範囲記法, "E010", "tests/mutants/m_e010.rule", "E010");
golden!(e011_別名, "E011", "tests/mutants/m_e011.rule", "E011");

// Labels and tables that share an output (§15.66).
golden!(e034_ラベル重複, "E034", "tests/mutants/m_e034.rule", "E034");
golden!(e035_指す先が無い, "E035", "tests/mutants/m_e035.rule", "E035");
golden!(w117_効かない例外, "W117", "tests/mutants/m_w117.rule", "W117");
golden!(e046_節の形, "E046", "tests/mutants/m_e046.rule", "E046");
golden!(e037_固定が無い, "E037", "tests/mutants/m_e037.rule", "E037");
golden!(e038_コピーが変わった, "E038", "tests/mutants/m_e038.rule", "E038");
golden!(e039_コピーが無い, "E039", "tests/mutants/m_e039.rule", "E039");
golden!(w119_引かれていない固定, "W119", "tests/mutants/m_w119.rule", "W119");

// The rows against the copy they were transcribed from (§15.82).
golden!(e049_桁区切りのカンマ, "E049", "tests/mutants/m_e049.rule", "E049");
golden!(e116_コピーに無い金額, "E116", "tests/mutants/m_e116.rule", "E116");
golden!(w120_転記し忘れた行, "W120", "tests/mutants/m_w120.rule", "W120");
golden!(e119_境界の側, "E119", "tests/mutants/m_e119.rule", "E119");
golden!(e119_列の見出しの側, "E119-col", "tests/mutants/m_e119col.rule", "E119");
golden!(e120_射影の型, "E120", "tests/mutants/m_e120.rule", "E120");
golden!(e121_契約に無いパス, "E121", "tests/mutants/m_e121.rule", "E121");
golden!(e122_契約が通すのに規則が受け付けない, "E122", "tests/mutants/m_e122.rule", "E122");
golden!(w123_契約が送らない値でしか当たらない行, "W123", "tests/mutants/m_w123.rule", "W123");
golden!(e123_契約が制約を約束しない, "E123", "tests/mutants/m_e123.rule", "E123");
golden!(w124_契約が送らない組み合わせでしか当たらない行, "W124", "tests/mutants/m_w124.rule", "W124");

// A rule applied by another (§15.69).
golden!(e040_呼び先が変わった, "E040", "tests/mutants/m_e040.rule", "E040");
golden!(e041_束縛が足りない, "E041", "tests/mutants/m_e041.rule", "E041");
golden!(e042_列挙の対応の欠け, "E042", "tests/mutants/m_e042.rule", "E042");
golden!(e043_範囲の外, "E043", "tests/mutants/m_e043.rule", "E043");
golden!(e044_呼び出せない, "E044", "tests/mutants/m_e044.rule", "E044");
golden!(w118_使われない表, "W118", "tests/mutants/m_w118.rule", "W118");

// A rule that is one step of a state machine (§15.148): each claim broken, with the shortest
// sequence of calls that breaks it, and a scenario that no longer holds.
golden!(e124_終わった案件が動く, "E124", "tests/mutants/m_e124.rule", "E124");
golden!(e125_終われない状態, "E125", "tests/mutants/m_e125.rule", "E125");
golden!(e126_取消のあとの出荷, "E126", "tests/mutants/m_e126.rule", "E126");
golden!(e127_二度目の返金, "E127", "tests/mutants/m_e127.rule", "E127");
golden!(e107_手順の例, "E107-scenario", "tests/mutants/m_e126.rule", "E107");
golden!(w125_届かない状態, "W125", "tests/mutants/m_w125.rule", "W125");

// The values a machine carries besides its state (§15.217): one with no start, a number that
// leaves its range, an `always` line broken, and a claim the intervals leave unsettled.
golden!(e072_持ち越す値の始まりが無い, "E072", "tests/mutants/m_e072ja.rule", "E072");
golden!(e134_範囲を出る試行回数, "E134", "tests/mutants/m_e134ja.rule", "E134");
golden!(e135_あきらめたときの回数, "E135", "tests/mutants/m_e135ja.rule", "E135");
golden!(w127_区間で決めきれない, "W127", "tests/mutants/m_w127ja.rule", "W127");

// The prerequisites of a course (§15.222): a course with no line, a cycle, a course that waits on
// one withdrawn from the catalog, one taught before what it needs, and a prerequisite another
// already needs.
golden!(e071_行の無い科目, "E071", "tests/mutants/m_e071ja.rule", "E071");
golden!(e136_前提の循環, "E136", "tests/mutants/m_e136ja.rule", "E136");
golden!(e137_たどり着けない科目, "E137", "tests/mutants/m_e137ja.rule", "E137");
golden!(e138_年次の食い違い, "E138", "tests/mutants/m_e138ja.rule", "E138");
golden!(w130_余分な前提, "W130", "tests/mutants/m_w130ja.rule", "W130");

// The walk's stages and the words of a cell (§15.208), and the rows nothing decides (§15.209).
golden!(e075_値でも名前でもある語, "E075", "tests/mutants/m_e075ja.rule", "E075");
golden!(e031_要素ごとの値の出力, "E031-walk", "tests/mutants/m_e031walkja.rule", "E031");
golden!(w128_決められない行, "W128", "tests/mutants/m_w128ja.rule", "W128");

// Two sequences and a largest value (§15.219): one item that reads both, and a `max` with no `empty`.
golden!(e031_二つの並び, "E031", "tests/mutants/m_e031ja.rule", "E031");
golden!(e022_最大の空, "E022-max", "tests/mutants/m_e022maxja.rule", "E022");

// Arithmetic (§15.213 to §15.216).
golden!(e132_割る数が0になりうる, "E132", "tests/mutants/m_e132ja.rule", "E132");
golden!(e139_平方根の引数が負になりうる, "E139", "tests/mutants/m_e139ja.rule", "E139");
golden!(e115_丸めの外の割り算, "E115-div", "tests/mutants/m_e115divja.rule", "E115");
golden!(e115_余りの割る数, "E115-mod", "tests/mutants/m_e115modja.rule", "E115");

// What a rule reads out of the caller's object (§15.223): a `from max` with no `empty`, and a
// sequence taken from a collection the contract does not have.
golden!(e074_最大の空, "E074", "tests/mutants/m_e074ja.rule", "E074");
golden!(e121_契約に無い並び, "E121-seq", "tests/mutants/m_e121seqja.rule", "E121");
