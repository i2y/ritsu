//! Acceptance conditions for M0 (§13).
//!
//! 1. The whole corpus passes check.
//! 2. Mutant files, each seeded with a single error, emit exactly the code decided for them.

use std::path::Path;

const CORPUS: &[&str] = &[
    "tests/corpus/品番の扱い.rule",
    "tests/corpus/買物かごの送料.rule",
    "tests/corpus/比例配分.rule",
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
        ("m_e101en.rule", &[("E101", 1)], "英語の規則から行を一つ落とした（README が見せる診断）"),
        ("m_e101c.rule", &[("E101", 1)], "constraint の陰に隠れた穴（§15.98。0 で埋めた点だけ見ていた）"),
        ("m_e101d.rule", &[("E101", 1), ("E107", 1)], "日付の境界に穴を開けた"),
        ("m_e102.rule", &[("E102", 1), ("W105", 6)], "catch-all の後ろに行を足した"),
        ("m_e102b.rule", &[("E102", 3), ("E107", 2)], "上流が出さない値を下流が名指しした"),
        ("m_e102c.rule", &[("E102", 1)], "上流の二つの表が同時には出さない組を名指しした（§15.114）"),
        ("m_e101e.rule", &[("E101", 1), ("E107", 2)], "起こりうる組を覆わずに残した（上流の篩が刈りすぎていないことの裏）"),
        ("m_e103.rule", &[("E103", 1)], "長さの列に金額を書いた"),
        ("m_e104.rule", &[("E104", 1)], "出力の丸め宣言を消した"),
        ("m_e104b.rule", &[("E104", 1)], "端数の出る式から丸めを消した"),
        ("m_e105.rule", &[("E101", 1), ("E105", 1)], "一意の表で行を重ねた"),
        ("m_e106.rule", &[("E106", 2)], "丸めの刻みに載らない額を書いた"),
        ("m_e107.rule", &[("E107", 1)], "例の期待値をずらした"),
        ("m_e108.rule", &[("E108", 1)], "入力の範囲を int64 に収まらないほど広げた"),
        ("m_e111.rule", &[("E111", 1)], "例から出力の列を落とした"),
        ("m_e112.rule", &[("E112", 1)], "導出の範囲を到達区間より狭くした"),
        ("m_e113.rule", &[("E113", 1)], "表の出力どうしを比べる真偽定義を書いた"),
        ("m_w105.rule", &[("W105", 3)], "上からの表で出力の食い違う重なりを作った"),
        ("m_w111.rule", &[("W111", 1)], "contract_only の印を消した"),
        ("m_e034.rule", &[("E034", 1)], "同じ表の二つの行に同じラベルを付けた"),
        ("m_e035.rule", &[("E035", 1)], "overrides で無い表を指した"),
        ("m_w117.rule", &[("W117", 1)], "交わらない行への overrides を足した"),
        ("m_e046.rule", &[("E046", 1)], "節の when の行を消した"),
        ("m_e037.rule", &[("E037", 1)], "引いた断片の固定行を消した"),
        ("m_e038.rule", &[("E038", 1)], "固定のハッシュを写しと違うものにした"),
        ("m_e039.rule", &[("E039", 1)], "写しの無い断片を引いた"),
        ("m_w119.rule", &[("W119", 1)], "引いていない断片の固定行を足した"),
        ("m_e040.rule", &[("E040", 1)], "呼び先の固定ハッシュを実物と違うものにした"),
        ("m_e041.rule", &[("E041", 1)], "呼び先の入力を一つ束縛しなかった"),
        ("m_e042.rule", &[("E042", 1), ("W111", 1)], "列挙の対応から一つの値を落とした"),
        ("m_e043.rule", &[("E043", 1)], "入力の範囲を呼び先の範囲の外まで広げた"),
        ("m_e044.rule", &[("E044", 1)], "check を通らない規則を呼び出した"),
        ("m_w118.rule", &[("W118", 1)], "呼び先の表の全行に優先する節を足した"),
        ("m_e116.rule", &[("E116", 1), ("W120", 1)], "写しの 880円 を 890円 と写した（打ち間違いは二つ同時に出る）"),
        ("m_w120.rule", &[("W120", 1)], "写しの一行を写し忘れ、残った行がその入力を拾っている"),
        ("m_e119.rule", &[("E119", 2)], "写しの「Under 18」の側を取り違えた（境界を分け合う二行の両方が出る）"),
        ("m_e119col.rule", &[("E119", 2)], "写しが「円以上」「円未満」と列の見出しで言っている側を取り違えた"),
        ("m_e120.rule", &[("E120", 1)], "件数を受ける入力を bool のままにした"),
        ("m_e121.rule", &[("E121", 1)], "契約が持っていないフィールドを射影した（フィールドの名前が変わったときの姿）"),
        ("m_w122.rule", &[("W122", 1)], "契約を宣言したまま、どの入力も射影していない"),
        ("m_e122.rule", &[("E122", 1)], "契約は 50 件まで通すのに、入力は 40 件までしか受け付けない"),
        ("m_w123.rule", &[("W123", 1)], "契約が 50 件までしか通さないのに、60 件を超える行を書いた"),
        ("m_e123.rule", &[("E123", 1)], "契約は申告額が補償額と等しい要求を通すのに、制約を `<` にした"),
        ("m_w124.rule", &[("W124", 1)], "契約が 5kg を超える速達を通さないのに、その行を書いた"),
        ("m_w114.rule", &[("W114", 1)], "境界が導出の取れる値のあいだに落ちている（整数であることまでは見ていない）"),
        // §15.86. Six positions where a value meets a declared type and nobody compared
        // them. Each of these produced **nothing at all** until that entry: the corpus is
        // made of correct rules, so a position no check visits looks exactly like a position
        // that checks out. `tests/positions.rs` holds the whole table; these six are the ones
        // seeded into a real rule, where the consequence is a real amount.
        ("m_e103ex.rule", &[("E103", 1)], "例の期待値を違う単位で書いた（比較ごと飛ばされていた）"),
        ("m_e103round.rule", &[("E103", 1)], "丸めの格子を違う単位で書いた（1円 に落ちていた）"),
        ("m_e103fold.rule", &[("E103", 1)], "畳み込みの empty の答えを銭で書いた（円として読まれていた）"),
        ("m_e103elem.rule", &[("E103", 1)], "要素のフィールドの範囲を違う単位で書いた（診断が捨てられていた）"),
        ("m_e103step.rule", &[("E103", 1)], "型の刻みを違う単位で書いた（1 に落ちて範囲の目盛りが変わっていた）"),
        ("m_e012group.rule", &[("E012", 1)], "群の一員を打ち間違えた（黙って群から外れていた）"),
        // §15.88. The walk and the count had no mutant of any kind: their diagnostics were
        // exercised only by their own minimal examples in the ledger, never by a rule a
        // business would write.
        ("m_e021.rule", &[("E021", 1)], "`over` が宣言されていない並びを指している"),
        ("m_e022.rule", &[("E022", 1)], "空の並びのときの答えが無い"),
        ("m_e023.rule", &[("E023", 1)], "最後まで見終えたときの答えが無い"),
        ("m_e024.rule", &[("E024", 1)], "表が出す判定に行き先が無い"),
        ("m_e027.rule", &[("E027", 1), ("W116", 1)], "例が、宣言されていない並びを名指している"),
        ("m_e028.rule", &[("E028", 1)], "`count` に `over` が無い"),
        ("m_e029.rule", &[("E029", 1), ("W111", 1)], "`where` が要素の持たないフィールドを名指している"),
        ("m_e030.rule", &[("E030", 1)], "`count` に範囲が無い（完全性の全体集合と並びの上限を兼ねる）"),
        ("m_e031.rule", &[("E031", 1)], "同じ並びを畳みも数えもしている"),
        ("m_e014.rule", &[("E014", 1)], "出力のセルに式を書いた"),
        ("m_e016.rule", &[("E016", 1)], "`result` の行が二つある"),
        // This one used to **panic** `rulec check`: the interval of the derived value was
        // computed before the diagnostic, and a divisor whose range contains zero asserted
        // its way out of the process (§15.88).
        ("m_e115.rule", &[("E115", 1)], "割る数が定数でない（かつては検査器ごと落ちていた）"),
        ("m_e117.rule", &[("E117", 2)], "累計が全体を超えないと言う制約が無い配分（§15.102）"),
        ("m_e118.rule", &[("E118", 1), ("W111", 1)], "引数が一つ足りない呼び出し（§15.102）"),
        ("m_w121.rule", &[("W121", 1)], "生成先の予約語と同じ別名（§15.103）"),
        // §15.92. Five more codes that had no mutant. The syntax errors keep their minimal
        // example in the ledger — a misplaced character has no amount attached — but these
        // change what a real table answers, or what it is allowed to claim.
        ("m_e009.rule", &[("E009", 1), ("E012", 2), ("W111", 1)], "入力の名前を言語の語にした"),
        ("m_e015.rule", &[("E015", 1), ("E103", 1)], "`result` が最初でない出力を名指した"),
        ("m_e047.rule", &[("E047", 1)], "範囲の後ろに税区分を書いた（黙って捨てられていた）"),
        ("m_e049.rule", &[("E049", 1)], "閾値を文書のとおり桁区切りのカンマつきで写した（二つの値に読まれていた）"),
        ("m_e048.rule", &[("E048", 1), ("E112", 1), ("W111", 1)], "日付から日付を引いた"),
        ("m_e114.rule", &[("E114", 3)], "率をその列の刻みに載らない値にした"),
        ("m_w110.rule", &[("W105", 1), ("W110", 1)], "重ならない表に `policy first` を付けた"),
        // The last of the seedable ones. What is left keeps its minimal example in the
        // ledger and nothing more: E109 needs `--budget` (tests/cli.rs passes it), E110 needs
        // a column's type *and* its cells changed at once, and W114 needs two derived values
        // sharing an input — a shape no transcription has (§15.92).
        ("m_e025.rule", &[("E025", 1), ("W116", 2)], "例がどの並びを歩くのか言っていない"),
        ("m_e026.rule", &[("E026", 1)], "`sequence` の見出しが要素のフィールドと合っていない"),
        ("m_e036.rule", &[("E036", 1)], "`overrides` の相手が別の出力を定めている"),
        ("m_e045.rule", &[("E045", 1)], "出力を二つ決める表に、節が優先している"),
        ("m_w115.rule", &[("W111", 1), ("W115", 1)], "どの要素も landing しない判定に行き先がある"),
        // Two gates of one loop, with the contract beside them (tests/mutants/contracts/).
        // They are written by hand: the seed is in the `.proto`, not in a corpus rule.
        ("m_e032.rule", &[("E032", 1)], "契約の列挙に値が増え、規則がそれを知らない"),
        ("m_e033.rule", &[("E033", 1)], "増えた値に行も `default` も無い"),
        // §15.93. The syntax errors, seeded into a real rule. The ledger's minimal example is
        // four lines long and cannot show that the diagnostic lands on the right line of a
        // sixty-line table. Where a seed cascades it is pinned as it is: a lexer error does
        // leave the rest of the file headless, and pretending otherwise would be the fiction.
        ("m_e001.rule", &[("E001", 1)], "文字列を閉じなかった"),
        ("m_e002.rule", &[("E002", 1), ("E004", 5)], "識別子を始められない文字を置いた"),
        ("m_e003.rule", &[("E003", 1)], "一行目が `rule` でない"),
        ("m_e004.rule", &[("E004", 1), ("E005", 1)], "行の先頭に語が無い"),
        ("m_e005.rule", &[("E005", 1)], "その位置に書けない語を置いた"),
        ("m_e006.rule", &[("E006", 1)], "列挙の宣言から `=` を落とした"),
        ("m_e007.rule", &[("E007", 1)], "無い方式を `policy` に書いた"),
        ("m_e013.rule", &[("E013", 1)], "無い取込先を指した"),
        ("m_e017.rule", &[("E017", 1)], "`constraint` が関係の形をしていない"),
        ("m_e018.rule", &[("E018", 1)], "`constraint` の片側が表の出力"),
        ("m_e019.rule", &[("E019", 3)], "例が制約の外にある"),
        ("m_e020.rule", &[("E020", 1)], "`elements` に名前が無い"),
        // Two edits, because either alone is a different error. §11 calls E110 the internal
        // breakwater — what is confirmed is that it fires rather than a table being skipped.
        ("m_e110.rule", &[("E110", 1)], "検査器が畳めない型の列を作った"),
    ];

    // `m_e102b` used to carry an E101 as well, demanding a row for `可否 = true` — the very
    // value it had just been told the table above never produces, and the very rows E102 names
    // dead in the same run. The gap search reads the tables above now, so the two no longer
    // contradict each other.

    // Also close the gap of adding material but forgetting to add it to the table.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mutants");
    let on_disk = std::fs::read_dir(&dir)
        .expect("変異の置き場が無い")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rule"))
        .count();
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
fn 隠れは三つに分けられる() {
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
fn 隠れは上からの表でだけ出る() {
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
fn 共有する入力ごしの重なりは消去で決まる() {
    // §6.2 のふるいは導出ごとに独立な区間しか見ないので、入力を共有する二つの導出の結びつきが
    // 見えない。§15.126 の Fourier–Motzkin 消去がそれを決める。この規則の 行1 と 行2 は
    // 重なって見えるが、残高B <= 残高A なので同時には当たらない。
    let ds = codes("tests/corpus/クーポン併用.rule");
    assert!(!ds.iter().any(|c| c == "W114"), "消去で決まるので W114 は出ないはず");
    assert!(!ds.iter().any(|c| c == "E105"), "起きない重なりをエラーにしてはいけない");
    assert!(!ds.iter().any(|c| c == "E101"), "この表は完全なはず");
}

#[test]
fn 真偽の定義の中の閾値も消去に入る() {
    // §15.127: 重なりが真偽の定義を一つの値に決めているなら、その定義の本体は
    // そこで成り立たなければならない比較である。中の閾値が連立に入り、同じ入力から
    // 出た二つの定義が同時に真になれないことが決まる。
    let ds = inline(
        "rule t(t) v1\n\ninputs\n  a(a) : number  range >=0 <=10\n\noutputs\n  r(r) : bool\n\n\
         define 上(up) : bool = a >= 8\ndefine 下(dn) : bool = a <= 2\n\n\
         table j(j)\npolicy unique\n| 上 | 下 | -> r(r) : bool |\n\
         | true | - | true |\n| - | true | false |\n| false | false | false |\n",
    );
    assert!(!ds.iter().any(|c| c == "W114"), "消去で決まるので W114 は出ないはず: {ds:?}");
    assert!(!ds.iter().any(|c| c == "E105"), "起きない重なりをエラーにしてはいけない: {ds:?}");
}

#[test]
fn 有理数で解く限界は警告に落ちる() {
    // 残るのは、消去が有理数の上で解いているために決まらない形である。証明できていない
    // ものを証明済みとして出さないので、E105 ではなく W114 になる（§15.127 の正直な限界）。
    let ds = codes("tests/mutants/m_w114.rule");
    assert_eq!(ds.iter().filter(|c| *c == "W114").count(), 1, "W114 が一件出るはず");
    assert!(!ds.iter().any(|c| c == "E105"), "判定できない重なりをエラーにしてはいけない");
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
fn 定義の中で矛盾する重なりは消去が決める() {
    // For the same input, `>=3万円` and `<=1000円` cannot both hold. The region analysis
    // treats definitions as free axes and cannot eliminate the intersection on its own, and
    // no witness can be constructed either — it used to be demoted to W114 and a runtime
    // guard. §15.127 puts the thresholds inside the definitions into the system, so the
    // pair is decided and neither the warning nor the guard is written.
    let src = "rule t(t) v1\n\ninputs\n  金額(a) : money[円, incl_tax]  range >=0円 <=10万円\n\n\
               outputs\n  r(r) : bool\n\n\
               define 大口(bulk) : bool = 金額 >= 3万円\ndefine 小口(small) : bool = 金額 <= 1000円\n\n\
               table x(x)\npolicy unique\n| 大口 | 小口 | -> r(r) : bool |\n\
               | true   | -    | true |\n| -    | true   | false |\n| false   | false   | false |\n";
    let ds = rulec::check_source(src, "d.rule");
    let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
    assert!(!codes.contains(&"E105"), "起きない重なりでエラーにしている: {codes:?}");
    assert!(!codes.contains(&"W114"), "消去で決まるので警告も出ない: {codes:?}");
    let (f, c) = rulec::prepare(src, "d.rule").expect("検査は通る");
    let py = rulec::codegen::Gen::new(&f, &c, src).python();
    // The exception class is part of every module; what a decided pair does not get is the
    // guard, and the guard names the code it came from.
    assert!(!py.contains("W114"), "決まった対にガードは要らない");
}

#[test]
fn 決められない重なりはガードへ降ろす() {
    // What the elimination cannot decide it does not claim: it solves over the rationals, so
    // a pair kept apart only by the values being whole stays unconfirmed. Not an error, and
    // **not a proof of nonexistence** either — so the generated code gets a runtime guard.
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mutants/m_w114.rule"),
    )
    .unwrap();
    let ds = rulec::check_source(&src, "d.rule");
    assert!(!ds.iter().any(|d| d.code == "E105"), "構成できない重なりでエラーにしている");
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
    let (f, c) = rulec::prepare(&src, "d.rule").expect("検査は通る");
    let py = rulec::codegen::Gen::new(&f, &c, &src).python();
    assert!(py.contains("ガード"), "ガードが入っていない");
    assert!(py.contains("RuleContradictionError"), "ガードが例外を投げない");
}

#[test]
fn 解析できない型の列は黙って飛ばさない() {
    // When a type could not be analyzed, `TableRegion::build` gave up and ok was printed with
    // neither the completeness nor the duplication of that table checked. Having stepped on this
    // twice, with dates and with optional, we stop with E110 as a general breakwater (§6.3).
    let src = "rule t(t) v1\n\ninputs\n  s(s) : string?\n\noutputs\n  r(r) : bool\n\n\
               table x(x)\npolicy unique\n| s | -> r(r) : bool |\n| starts_with \"a\" | true |\n";
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

/// The keyword table on the site must match the words the parser actually accepts.
/// Both adding a word without writing it in the table, and listing a word the parser lacks, go red.
#[test]
fn 文書のキーワード表はパーサと一致する() {
    let md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("website/docs-ja/tour.md"),
    )
    .unwrap();
    let i = md.find("行頭に書けるのは次の語だけで").expect("キーワード表の前書きが無い");
    let j = md[i..].find("\n\n## ").expect("表の終わりが無い") + i;
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

/// The mutants are a build product, and this is what holds them to their source.
///
/// `tests/make-mutants.sh` seeds one error into a corpus rule. The files it writes are
/// committed — fourteen test files read them — but nothing ran the script, so **a corpus rule
/// that moved on left a mutant that was quietly no longer that rule with one seeded error**
/// (§15.92). `m0` would keep passing: it checks the codes of the files on disk, whatever they
/// have become. The same shape as the committed `rulec.wasm`, which does have such a test.
///
/// A few mutants are written by hand rather than seeded — the two that need a document beside
/// them (E116, W120) and the copies under `sources/`. They are named here so that "not
/// generated" is a decision rather than a gap.
#[test]
fn 変異はコーパスから作り直せる() {
    // `m_e101c.rule` is written here rather than cut from a corpus rule: the shape it needs
    // is a `constraint` whose forbidden corner hides an uncovered box (§15.98), and no
    // corpus rule has one.
    const BY_HAND: &[&str] = &[
        "m_e116.rule",
        "m_w120.rule",
        "m_e032.rule",
        "m_e033.rule",
        "m_e101c.rule",
        // §15.126: the shape that is left over once the elimination has decided the
        // arithmetic ones is two boolean definitions off one input, and no corpus rule has
        // one — the corpus is made of rules that are right.
        "m_w114.rule",
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp = std::env::temp_dir().join(format!("rulec-mutants-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let out = std::process::Command::new("sh")
        .current_dir(root)
        .env("RULEC", env!("CARGO_BIN_EXE_rulec"))
        .arg("tests/make-mutants.sh")
        .arg(&tmp)
        .output()
        .expect("make-mutants.sh を起動できない");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let names = |d: &std::path::Path| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".rule"))
            .collect();
        v.sort();
        v
    };
    let (have, made) = (names(&root.join("tests/mutants")), names(&tmp));
    let seeded: Vec<String> =
        have.iter().filter(|n| !BY_HAND.contains(&n.as_str())).cloned().collect();
    assert_eq!(
        seeded, made,
        "変異の顔ぶれが生成器と違います。足したなら make-mutants.sh にも足し、\
         手で置いたものなら BY_HAND に名前を書いてください"
    );
    for n in &made {
        let a = std::fs::read_to_string(root.join("tests/mutants").join(n)).unwrap();
        let b = std::fs::read_to_string(tmp.join(n)).unwrap();
        assert_eq!(
            a, b,
            "{n} がコーパスから作り直したものと違います。\
             コーパスを直したなら `sh tests/make-mutants.sh` で焼き直してください"
        );
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

/// The contracts beside the mutants are copies of the corpus's, so that a mutant reaches its
/// contract by the relative path the corpus rule it was cut from uses. A copy that drifted
/// would hold a mutant to a contract its rule never saw.
#[test]
fn 変異の隣の契約はコーパスの写しと同じ() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut compared = 0;
    for e in std::fs::read_dir(root.join("tests/mutants/contracts")).unwrap().flatten() {
        let corpus = root.join("tests/corpus/contracts").join(e.file_name());
        if !corpus.exists() {
            continue;
        }
        let (a, b) = (std::fs::read_to_string(e.path()).unwrap(), std::fs::read_to_string(&corpus).unwrap());
        assert!(a == b, "{} がコーパスの写しと違う", e.path().display());
        compared += 1;
    }
    assert!(compared >= 2, "比べた契約が {compared} 本しかない");
}
