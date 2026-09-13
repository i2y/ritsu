//! Acceptance conditions for M0 (§13).
//!
//! 1. The whole corpus passes check.
//! 2. Mutant files, each seeded with a single error, emit exactly the code decided for them.

use std::path::Path;

const CORPUS: &[&str] = &[
    "tests/corpus/ゆうパック運賃.rule",
    "tests/corpus/クーポン割引.rule",
    "tests/corpus/クーポン併用.rule",
    "tests/corpus/送料.rule",
    "tests/corpus/期間区分.rule",
    "tests/corpus/適用順序.rule",
    "tests/corpus/クーポン一枚.rule",
];

fn check(rel: &str) -> Vec<(String, String)> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let src = std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("読めない: {rel}"));
    rulec::check_source(&src, rel)
        .iter()
        .map(|d| (d.code.to_string(), d.title.clone()))
        .collect()
}

fn codes(rel: &str) -> Vec<String> {
    check(rel).into_iter().map(|(c, _)| c).collect()
}

#[test]
fn コーパスは全部通る() {
    for f in CORPUS {
        let ds = check(f);
        let bad: Vec<_> = ds.iter().filter(|(c, _)| c.starts_with('E')).collect();
        assert!(bad.is_empty(), "{f} にエラーが出た: {bad:?}");
    }
}

#[test]
fn 変異は決めたコードだけを出す() {
    // Pin the **full set** of diagnostics each mutant file emits, down to the counts.
    //
    // Previously only "contains E112" was checked. Because of that, a mutant that had not followed
    // a rename in the corpus (`enum 範囲` → `enum 適用範囲`, found through E009) and was emitting E009
    // as well still passed: golden picked out only E112 and returned green. Unless an extra
    // diagnostic turns the test red, nobody notices when the material rots.
    //
    // (mutant file, every diagnostic it emits, what was broken)
    let cases: &[(&str, &[(&str, usize)], &str)] = &[
        ("m_e008.rule", &[("E008", 7)], "セルを空にした"),
        ("m_e010.rule", &[("E010", 1)], "`..` を書いた"),
        ("m_e011.rule", &[("E011", 1)], "公開面の別名を消した"),
        ("m_e101.rule", &[("E101", 1)], "群から 山梨県 を落とした"),
        ("m_e101d.rule", &[("E101", 1), ("E107", 1)], "日付の境界に穴を開けた"),
        ("m_e102.rule", &[("E102", 1), ("W105", 6)], "catch-all の後ろに行を足した"),
        ("m_e102b.rule", &[("E101", 1), ("E102", 3), ("E107", 2)], "上流が出さない値を下流が名指しした"),
        ("m_e103.rule", &[("E103", 1)], "長さの列に金額を書いた"),
        ("m_e104.rule", &[("E104", 1)], "出力の丸め宣言を消した"),
        ("m_e104b.rule", &[("E104", 1)], "端数の出る式から丸めを消した"),
        ("m_e105.rule", &[("E101", 1), ("E105", 1)], "一意の表で行を重ねた"),
        ("m_e106.rule", &[("E106", 2)], "丸めの格子に載らない額を書いた"),
        ("m_e107.rule", &[("E107", 1)], "例の期待値をずらした"),
        ("m_e108.rule", &[("E108", 1)], "入力の範囲を int64 に収まらないほど広げた"),
        ("m_e111.rule", &[("E111", 1)], "例から出力の列を落とした"),
        ("m_e112.rule", &[("E112", 1)], "導出の範囲を到達区間より狭くした"),
        ("m_e113.rule", &[("E113", 1)], "表の出力どうしを比べる真偽定義を書いた"),
        ("m_w105.rule", &[("W105", 3)], "上からの表で出力の食い違う重なりを作った"),
        ("m_w111.rule", &[("W111", 1)], "contract_only の印を消した"),
    ];

    // Also close the gap of adding material but forgetting to add it to the table.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mutants");
    let on_disk = std::fs::read_dir(&dir).expect("変異の置き場が無い").count();
    assert_eq!(on_disk, cases.len(), "変異ファイルの数と、固定した数が合わない");

    for (f, want, what) in cases {
        let rel = format!("tests/mutants/{f}");
        let mut got: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for c in codes(&rel) {
            *got.entry(c).or_insert(0) += 1;
        }
        let want: std::collections::BTreeMap<String, usize> =
            want.iter().map(|(c, n)| (c.to_string(), *n)).collect();
        assert_eq!(got, want, "{f}（{what}）が出す診断が変わった");
    }
}

#[test]
fn 遮蔽は三分類される() {
    // §4: structural and equivalent get counts only; only needs-confirmation pairs are listed.
    let r = |rel: &str| {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let src = std::fs::read_to_string(&p).unwrap();
        rulec::report(&src, rel)
    };

    let y = r("tests/corpus/ゆうパック運賃.rule").shadow;
    assert_eq!((y.structural, y.equivalent, y.confirm), (21, 0, 0), "階段は全部構造的");

    let k = r("tests/corpus/クーポン割引.rule").shadow;
    assert_eq!((k.structural, k.equivalent, k.confirm), (4, 6, 0), "守り行は同値");

    // Making the outputs disagree raises needs-confirmation.
    let m = r("tests/mutants/m_w105.rule");
    assert!(m.shadow.confirm > 0, "出力が違う部分交差は要確認になるはず");
    assert!(
        m.diags.iter().any(|d| d.code == "W105"),
        "要確認は既定で一覧に出るはず"
    );
}

#[test]
fn 遮蔽は上からの表でだけ出る() {
    // W105 does not appear in a `unique` table (an overlap there is an error).
    let ds = codes("tests/corpus/ゆうパック運賃.rule");
    assert!(!ds.iter().any(|c| c == "E105"), "上から の重なりがエラーになってはいけない");
}

#[test]
fn 例は実行される仕様である() {
    // Every example in the corpus must hit. A miss emits E107.
    for f in CORPUS {
        assert!(!codes(f).iter().any(|c| c == "E107"), "{f} の例が外れた");
    }
}

#[test]
fn 篩が判定できない重なりは警告に落ちる() {
    // §6.2: when two derivations share an input, the sieve of independent intervals cannot see the
    // dependency. Nothing unproven is presented as proven, so it is W114, not E105.
    let ds = codes("tests/corpus/クーポン併用.rule");
    assert_eq!(ds.iter().filter(|c| *c == "W114").count(), 1, "W114 が一件出るはず");
    assert!(!ds.iter().any(|c| c == "E105"), "判定できない重なりをエラーにしてはいけない");
    assert!(!ds.iter().any(|c| c == "E101"), "この表は完全なはず");
}

/// Check an inline source and return the codes it emitted.
fn inline(src: &str) -> Vec<String> {
    rulec::check_source(src, "inline.rule")
        .iter()
        .map(|d| d.code.to_string())
        .collect()
}

const HEAD: &str = "rule 試し(t) v1\n\ninputs\n  x(x) : bool\n\noutputs\n  r(r) : bool\n\n";

#[test]
fn 構文側の台帳も全部鳴る() {
    let cases: &[(&str, &str, &str)] = &[
        ("E001", "rule 試し(t) v1\ndescription \"閉じない\n", "文字列が閉じていない"),
        ("E003", "description \"規則で始まらない\"\n", "rule の行で始まらない"),
        ("E005", "rule 試し(t) v1\n知らない語 x\n", "この位置で知らない語"),
        ("E006", "rule 試し(t) v1\nenum 区分(k) 甲(a)\n", "宣言に = がない"),
        ("E007", "rule 試し(t) v1\n\ntable t(t)\npolicy なんとか\n| x | -> r(r) : bool |\n| - | true |\n", "知らない方式"),
        ("E012", &format!("{HEAD}result r = 知らない名前\n"), "宣言されていない名前"),
        ("E013", "rule 試し(t) v1\nimport std/ありません\n", "取込先が無い"),
        ("E004", "rule 試し(t) v1\n= 語で始まらない\n", "行頭に語がない"),
        ("E002", "rule 試し(t) v1\n\u{7}\n", "制御文字は名前になれない"),
    ];
    for (want, src, what) in cases {
        let got = inline(src);
        assert!(got.iter().any(|c| c == want), "{what}: {want} が出ず {got:?}");
    }
}

#[test]
fn 重なりのない上からは一意を勧める() {
    // W110: if the order carries no meaning, `unique` can guarantee that reordering does not
    // change the meaning.
    let src = format!(
        "{HEAD}table t(t)\npolicy first\n| x  | -> r(r) : bool |\n| true | true |\n| false | false |\n"
    );
    assert!(inline(&src).iter().any(|c| c == "W110"), "{:?}", inline(&src));
}

#[test]
fn 真偽定義の列が解析される() {
    // The sketch from §1.2. The happy path of putting a boolean definition in a column.
    // 負担判定 is `first`, and row 1 (大口) and row 2 (プラチナ) partially intersect with
    // differing outputs.
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/送料.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    let r = rulec::report(&src, "送料.rule");
    assert!(!rulec::has_error(&r.diags), "{:?}", r.diags.iter().map(|d| d.code).collect::<Vec<_>>());
    assert_eq!(
        (r.shadow.structural, r.shadow.equivalent, r.shadow.confirm),
        (2, 0, 1),
        "0% と 50% の部分交差が要確認で残るはず"
    );
}

#[test]
fn 日付の列が解析される() {
    // Dates are held as ordinals and reuse the comparison and range machinery as is (§2.1).
    // Opening a hole at a boundary makes that calendar day appear as the witness.
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mutants/m_e101d.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    let ds = rulec::check_source(&src, "m.rule");
    assert!(ds.iter().any(|d| d.code == "E101"), "日付の穴を捕まえるはず");
    let e101 = ds.iter().find(|d| d.code == "E101").unwrap();
    assert!(
        e101.notes.iter().any(|n| n.contains("2026-04-01")),
        "証人が暦日で出るはず: {:?}",
        e101.notes
    );
}

#[test]
fn 両端含みの敷き詰めは穴を作らない() {
    // Holding dates as y*10000+m*100+d leaves a gap of nonexistent integers between the end of a
    // month and the start of the next, so a tiling of `<=2026-03-31` and `>=2026-04-01` emits a
    // false E101. It goes away by holding dates as day counts and never creating an open interval
    // between adjacent boundaries (§2.1).
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/期間区分.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    let ds = rulec::check_source(&src, "期間区分.rule");
    assert!(
        !ds.iter().any(|d| d.code == "E101"),
        "両端含みの敷き詰めに穴は無いはず: {:?}",
        ds.iter().map(|d| d.code).collect::<Vec<_>>()
    );
}

#[test]
fn 日付どうしの比較が原子として通る() {
    // The second atom of §5.3. Types that cannot be made into derivations (dates, enums) may be
    // compared with each other directly.
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/適用順序.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    let ds = rulec::check_source(&src, "適用順序.rule");
    assert!(!ds.iter().any(|d| d.code == "E113"), "日付どうしの比較は通る");
    assert!(!ds.iter().any(|d| d.code == "E107"), "例が全部当たる");

    // Comparing numbers directly stays E113, since as a derivation it can be analyzed exactly.
    let bad = src.replace("A期限 <= B期限", "A期限 <= B期限");
    let _ = bad;
    let num = "rule t(t) v1\n\ninputs\n  a(a) : money[円, incl_tax] range >=0円 <=100円\n  b(b) : money[円, incl_tax] range >=0円 <=100円\n\noutputs\n  r(r) : bool\n\ndefine x(x) : bool = a <= b\n";
    assert!(
        rulec::check_source(num, "n.rule").iter().any(|d| d.code == "E113"),
        "金額どうしの直接比較は E113 のまま"
    );
}

#[test]
fn 刻み隣接の空座標は三つの型で作られない() {
    // If adjacent boundaries differ by exactly one step, there is no value between them. Creating
    // an empty coordinate makes a closed-interval tiling emit a false E101 (§6.3). Not only dates
    // but money and rates go through the same arm, so all three are pinned. The bug had been
    // lurking since before dates existed.

    // Money: <=1000円 and >=1001円 are adjacent (the runtime representation is a single integer in
    // yen).
    let money = "rule t(t) v1\n\ninputs\n  a(a) : money[円, incl_tax]  range >=0円 <=10000円\n\n\
                 outputs\n  r(r) : bool\n\ntable x(x)\npolicy unique\n\
                 | a        | -> r(r) : bool |\n| <=1000円 | true |\n| >=1001円 | false |\n";
    let ds = rulec::check_source(money, "money.rule");
    assert!(
        !ds.iter().any(|d| d.code == "E101"),
        "金額の隣接に穴は無いはず: {:?}",
        ds.iter().map(|d| d.code).collect::<Vec<_>>()
    );

    // Rate: with a step of 1%, <=10% and >=11% are adjacent.
    let rate = "rule t(t) v1\n\ninputs\n  a(a) : rate[step 1%]  range >=0% <=100%\n\n\
                outputs\n  r(r) : bool\n\ntable x(x)\npolicy unique\n\
                | a      | -> r(r) : bool |\n| <=10%  | true |\n| >=11%  | false |\n";
    let ds = rulec::check_source(rate, "rate.rule");
    assert!(
        !ds.iter().any(|d| d.code == "E101"),
        "率の隣接に穴は無いはず: {:?}",
        ds.iter().map(|d| d.code).collect::<Vec<_>>()
    );

    // Conversely, a gap wider than the step shows up as a hole (the control that the check works).
    let hole = money.replace(">=1001円", ">=1002円");
    assert!(
        rulec::check_source(&hole, "hole.rule").iter().any(|d| d.code == "E101"),
        "一円分の穴は捕まえるはず"
    );
}

#[test]
fn 定義が絡む実在の重なりは入力を構成して示す() {
    // §6.2 "witness on a definition axis": region analysis merely places definitions as free axes,
    // so the coordinates of an intersection box need not actually exist. Construct an input, let
    // the evaluator compute the definitions too, and treat only what could be constructed as a
    // real contradiction (E105).
    let src = "rule t(t) v1\n\ninputs\n  金額(a) : money[円, incl_tax]  range >=0円 <=10000円\n  区分(b) : bool\n\n\
               outputs\n  r(r) : bool\n\ndefine 大口(bulk) : bool = 金額 >= 3000円\n\n\
               table x(x)\npolicy unique\n| 大口 | 区分 | -> r(r) : bool |\n\
               | true   | -    | true |\n| -    | true   | false |\n| false   | false   | false |\n";
    let ds = rulec::check_source(src, "d.rule");
    let e105 = ds.iter().find(|d| d.code == "E105").expect("実在する重なりなので E105");
    let built = e105
        .notes
        .iter()
        .find(|n| n.starts_with("この例を作る入力:"))
        .expect(&format!("構成した入力を出すはず: {:?}", e105.notes));
    assert!(built.contains("金額"), "写せる形で入力を出す: {built}");
    assert!(
        !e105.notes.iter().any(|n| n.contains("整合を確認していません")),
        "確かめたのに未確認だと言っている: {:?}",
        e105.notes
    );
}

#[test]
fn 定義が矛盾する重なりは番人へ降ろす() {
    // For the same input, `>=3万円` and `<=1000円` cannot both hold, but region analysis treats
    // definitions as free axes and so cannot eliminate this intersection. Since no witness can be
    // constructed, even under `unique` it is not an error; it is demoted to W114 and a guard (an
    // unproven existence must not stop CI; §6.2). It is **not a proof of nonexistence**, so the
    // generated code gets a runtime guard.
    let src = "rule t(t) v1\n\ninputs\n  金額(a) : money[円, incl_tax]  range >=0円 <=10万円\n\n\
               outputs\n  r(r) : bool\n\n\
               define 大口(bulk) : bool = 金額 >= 3万円\ndefine 小口(small) : bool = 金額 <= 1000円\n\n\
               table x(x)\npolicy unique\n| 大口 | 小口 | -> r(r) : bool |\n\
               | true   | -    | true |\n| -    | true   | false |\n| false   | false   | false |\n";
    let ds = rulec::check_source(src, "d.rule");
    assert!(
        !ds.iter().any(|d| d.code == "E105"),
        "構成できない重なりでエラーにしている: {:?}",
        ds.iter().map(|d| &d.code).collect::<Vec<_>>()
    );
    let w = ds.iter().find(|d| d.code == "W114").expect("W114 に降りるはず");
    assert!(
        w.notes.iter().any(|n| n.contains("構成できませんでした")),
        "構成できなかったことを言う: {:?}",
        w.notes
    );
    assert!(
        w.notes.iter().any(|n| n.contains("証明ではありません")),
        "非存在を断定していないことを言う: {:?}",
        w.notes
    );
    // The matching guard goes into the generated code (§8.1).
    let (f, c) = rulec::prepare(src, "d.rule").expect("検査は通る");
    let py = rulec::codegen::Gen::new(&f, &c, src).python();
    assert!(py.contains("番人"), "番人が入っていない");
    assert!(py.contains("RuleContradictionError"), "番人が例外を投げない");
}

#[test]
fn 解析できない型の列は黙って飛ばさない() {
    // When a type could not be analyzed, `TableRegion::build` gave up and ok was printed with
    // neither the completeness nor the duplication of that table checked. Having stepped on this
    // twice, with dates and with optional, we stop with E110 as a general breakwater (§6.3).
    let src = "rule t(t) v1\n\ninputs\n  s(s) : string\n\noutputs\n  r(r) : bool\n\n\
               table x(x)\npolicy unique\n| s | -> r(r) : bool |\n| \"a\" | true |\n";
    let ds = rulec::check_source(src, "s.rule");
    assert!(ds.iter().any(|d| d.code == "E110"), "解析できない列は E110: {:?}",
            ds.iter().map(|d| d.code).collect::<Vec<_>>());
}

#[test]
fn optionalの列も検査される() {
    // `T?` is treated as an enum with one extra value, "none". Opening a hole emits E101.
    let base = "rule t(t) v1\n\nenum 区分(k) = 甲(a) | 乙(b)\n\n\
                inputs\n  金額(amt) : money[円, incl_tax]  range >=0円 <=100万円\n  任意値(opt) : 区分?\n\n\
                outputs\n  r(r) : bool\n\ntable x(x)\npolicy unique\n\
                | 金額      | 任意値   | -> r(r) : bool |\n\
                | <1000円   | none     | true |\n\
                | <1000円   | 甲, 乙 | false |\n\
                | >=1000円  | -        | false |\n";
    let ds = rulec::check_source(base, "o.rule");
    assert!(!rulec::has_error(&ds), "完全な表は通る: {:?}",
            ds.iter().map(|d| format!("{}:{}", d.code, d.title)).collect::<Vec<_>>());

    // Dropping the "none" row opens a hole. Had the column been skipped, nobody would notice.
    let holed = base.replace("| <1000円   | none     | true |\n", "");
    let ds = rulec::check_source(&holed, "o.rule");
    assert!(ds.iter().any(|d| d.code == "E101"), "none の穴を捕まえるはず: {:?}",
            ds.iter().map(|d| d.code).collect::<Vec<_>>());
}

/// The keyword table in the README must match the words the parser actually accepts.
/// Both adding a word without writing it in the table, and listing a word the parser lacks, go red.
#[test]
fn readmeのキーワード表はパーサと一致する() {
    let md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"),
    )
    .unwrap();
    let i = md.find("行頭に書けるのは次の語だけで").expect("キーワード表の前書きが無い");
    let j = md[i..].find("\n\n### ").expect("表の終わりが無い") + i;
    let mut listed: Vec<String> = Vec::new();
    for l in md[i..j].lines().filter(|l| l.starts_with("| `")) {
        for part in l.split('|').next_or_all() {
            for w in part.split('`').skip(1).step_by(2) {
                listed.push(w.to_string());
            }
        }
    }
    listed.sort();
    listed.dedup();
    // `rule` is header-only and not in the parser's list, so add it here before comparing.
    let mut want: Vec<String> =
        rulec::parse::KEYWORDS.iter().map(|s| s.to_string()).chain([rulec::kw::RULE.to_string()]).collect();
    want.sort();
    assert_eq!(listed, want, "README のキーワード表とパーサが食い違う");
}

/// A small helper for looking at only the first cell of a `| `語` | 説明 |` row.
trait FirstCell {
    fn next_or_all(self) -> Vec<String>;
}
impl<'a, I: Iterator<Item = &'a str>> FirstCell for I {
    fn next_or_all(mut self) -> Vec<String> {
        let _ = self.next(); // the empty string before the leading `|`
        self.next().map(|s| vec![s.to_string()]).unwrap_or_default()
    }
}
