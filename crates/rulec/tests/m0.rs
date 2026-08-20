//! M0 の受け入れ条件（§13）。
//!
//! 1. コーパス全部が check を通ること。
//! 2. 誤りを一つずつ仕込んだ変異ファイルが、決めたコードをそのまま出すこと。

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
    // 変異ファイルが出す診断の**全集合**を、件数まで固定する。
    //
    // 以前は「E112 を含む」しか見ていなかった。そのせいで、コーパスの改名
    // （`型 範囲` → `型 適用範囲`、E009 で見つけたもの）に追随せず古いままだった
    // 変異が E009 を一緒に出していても、golden は E112 だけを拾って緑を返していた。
    // 余分な一件が赤になる形にしておかないと、材料が腐っても誰も気づかない。
    //
    // (変異ファイル, 出る診断の全部, 何を壊したか)
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
        ("m_w111.rule", &[("W111", 1)], "契約のみ の印を消した"),
    ];

    // 材料が増えたのに表に足し忘れる、という抜けも塞ぐ。
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
    // §4: 構造的と同値は件数だけ、要確認だけが一覧に出る。
    let r = |rel: &str| {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let src = std::fs::read_to_string(&p).unwrap();
        rulec::report(&src, rel)
    };

    let y = r("tests/corpus/ゆうパック運賃.rule").shadow;
    assert_eq!((y.structural, y.equivalent, y.confirm), (21, 0, 0), "階段は全部構造的");

    let k = r("tests/corpus/クーポン割引.rule").shadow;
    assert_eq!((k.structural, k.equivalent, k.confirm), (4, 6, 0), "守り行は同値");

    // 出力を食い違わせると要確認が立つ。
    let m = r("tests/mutants/m_w105.rule");
    assert!(m.shadow.confirm > 0, "出力が違う部分交差は要確認になるはず");
    assert!(
        m.diags.iter().any(|d| d.code == "W105"),
        "要確認は既定で一覧に出るはず"
    );
}

#[test]
fn 遮蔽は上からの表でだけ出る() {
    // 一意 の表に W105 は出ない（重なりはエラーになる）。
    let ds = codes("tests/corpus/ゆうパック運賃.rule");
    assert!(!ds.iter().any(|c| c == "E105"), "上から の重なりがエラーになってはいけない");
}

#[test]
fn 例は実行される仕様である() {
    // コーパスの例が全部当たること。当たらなければ E107 が出る。
    for f in CORPUS {
        assert!(!codes(f).iter().any(|c| c == "E107"), "{f} の例が外れた");
    }
}

#[test]
fn 篩が判定できない重なりは警告に落ちる() {
    // §6.2: 二つの導出が入力を共有すると、独立な区間の篩は従属を見られない。
    // 証明していないことを証明済みの顔で出さないので、E105 ではなく W114。
    let ds = codes("tests/corpus/クーポン併用.rule");
    assert_eq!(ds.iter().filter(|c| *c == "W114").count(), 1, "W114 が一件出るはず");
    assert!(!ds.iter().any(|c| c == "E105"), "判定できない重なりをエラーにしてはいけない");
    assert!(!ds.iter().any(|c| c == "E101"), "この表は完全なはず");
}

/// 直書きのソースを検査して、出たコードを返す。
fn inline(src: &str) -> Vec<String> {
    rulec::check_source(src, "inline.rule")
        .iter()
        .map(|d| d.code.to_string())
        .collect()
}

const HEAD: &str = "規則 試し(t) v1\n\n入力\n  x(x) : 真偽\n\n出力\n  r(r) : 真偽\n\n";

#[test]
fn 構文側の台帳も全部鳴る() {
    let cases: &[(&str, &str, &str)] = &[
        ("E001", "規則 試し(t) v1\n説明 \"閉じない\n", "文字列が閉じていない"),
        ("E003", "説明 \"規則で始まらない\"\n", "規則 の行で始まらない"),
        ("E005", "規則 試し(t) v1\n知らない語 x\n", "この位置で知らない語"),
        ("E006", "規則 試し(t) v1\n型 区分(k) 甲(a)\n", "宣言に = がない"),
        ("E007", "規則 試し(t) v1\n\n表 t(t)\n方式 なんとか\n| x | → r(r) : 真偽 |\n| - | 真 |\n", "知らない方式"),
        ("E012", &format!("{HEAD}結果 r = 知らない名前\n"), "宣言されていない名前"),
        ("E013", "規則 試し(t) v1\n取込 標準/ありません\n", "取込先が無い"),
        ("E004", "規則 試し(t) v1\n= 語で始まらない\n", "行頭に語がない"),
        ("E002", "規則 試し(t) v1\n\u{7}\n", "制御文字は名前になれない"),
    ];
    for (want, src, what) in cases {
        let got = inline(src);
        assert!(got.iter().any(|c| c == want), "{what}: {want} が出ず {got:?}");
    }
}

#[test]
fn 重なりのない上からは一意を勧める() {
    // W110: 順序に意味が無いなら 一意 のほうが、並べ替えが意味を変えないことを保証できる。
    let src = format!(
        "{HEAD}表 t(t)\n方式 上から\n| x  | → r(r) : 真偽 |\n| 真 | 真 |\n| 偽 | 偽 |\n"
    );
    assert!(inline(&src).iter().any(|c| c == "W110"), "{:?}", inline(&src));
}

#[test]
fn 真偽定義の列が解析される() {
    // §1.2 のスケッチ。真偽の 定義 を列に置く正常系。
    // 負担判定は 上から で、行1（大口）と行2（プラチナ）が部分交差して出力が違う。
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
    // 日付は順序数で持ち、比較と範囲の機構をそのまま使う（§2.1）。
    // 境界に穴を開けると、その暦日が証人として出る。
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
    // 日付を y*10000+m*100+d で持つと、月末と翌月初のあいだに実在しない整数の
    // 隙間ができ、`<=2026-03-31` と `>=2026-04-01` の敷き詰めが偽の E101 を出す。
    // 通算日で持ち、隣接する境界のあいだの開区間を作らないことで消える（§2.1）。
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
    // §5.3 の第二の原子。導出にできない型（日付・列挙）どうしは直接比べられる。
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/適用順序.rule");
    let src = std::fs::read_to_string(&p).unwrap();
    let ds = rulec::check_source(&src, "適用順序.rule");
    assert!(!ds.iter().any(|d| d.code == "E113"), "日付どうしの比較は通る");
    assert!(!ds.iter().any(|d| d.code == "E107"), "例が全部当たる");

    // 数値どうしを直接比べるのは、導出にすれば厳密に解析できるので E113 のまま。
    let bad = src.replace("A期限 <= B期限", "A期限 <= B期限");
    let _ = bad;
    let num = "規則 t(t) v1\n\n入力\n  a(a) : 金額[円, 税込] 範囲 >=0円 <=100円\n  b(b) : 金額[円, 税込] 範囲 >=0円 <=100円\n\n出力\n  r(r) : 真偽\n\n定義 x(x) : 真偽 = a <= b\n";
    assert!(
        rulec::check_source(num, "n.rule").iter().any(|d| d.code == "E113"),
        "金額どうしの直接比較は E113 のまま"
    );
}

#[test]
fn 刻み隣接の空座標は三つの型で作られない() {
    // 隣り合う境界の差が刻みちょうどなら、そのあいだに値は無い。空の座標を作ると
    // 両端含みの敷き詰めが偽の E101 を出す（§6.3）。日付だけでなく、金額も率も
    // 同じ腕を通るので、三本とも固定する。日付以前から潜んでいたバグだった。

    // 金額: <=1000円 と >=1001円 は隣接する（実行時表現は円の整数一本）。
    let money = "規則 t(t) v1\n\n入力\n  a(a) : 金額[円, 税込]  範囲 >=0円 <=10000円\n\n\
                 出力\n  r(r) : 真偽\n\n表 x(x)\n方式 一意\n\
                 | a        | → r(r) : 真偽 |\n| <=1000円 | 真 |\n| >=1001円 | 偽 |\n";
    let ds = rulec::check_source(money, "money.rule");
    assert!(
        !ds.iter().any(|d| d.code == "E101"),
        "金額の隣接に穴は無いはず: {:?}",
        ds.iter().map(|d| d.code).collect::<Vec<_>>()
    );

    // 率: 刻み 1% なら <=10% と >=11% が隣接する。
    let rate = "規則 t(t) v1\n\n入力\n  a(a) : 率[刻み 1%]  範囲 >=0% <=100%\n\n\
                出力\n  r(r) : 真偽\n\n表 x(x)\n方式 一意\n\
                | a      | → r(r) : 真偽 |\n| <=10%  | 真 |\n| >=11%  | 偽 |\n";
    let ds = rulec::check_source(rate, "rate.rule");
    assert!(
        !ds.iter().any(|d| d.code == "E101"),
        "率の隣接に穴は無いはず: {:?}",
        ds.iter().map(|d| d.code).collect::<Vec<_>>()
    );

    // 逆に、刻みより広く空けたら穴として出る（検査が効いていることの対照）。
    let hole = money.replace(">=1001円", ">=1002円");
    assert!(
        rulec::check_source(&hole, "hole.rule").iter().any(|d| d.code == "E101"),
        "一円分の穴は捕まえるはず"
    );
}

#[test]
fn 定義が絡む実在の重なりは入力を構成して示す() {
    // §6.2「定義軸の証人」: 領域解析は定義を自由軸として置くだけなので、
    // 交差箱の座標が実在するとは限らない。入力を構成して評価器に定義まで
    // 計算させ、構成できたものだけを実在の矛盾（E105）として扱う。
    let src = "規則 t(t) v1\n\n入力\n  金額(a) : 金額[円, 税込]  範囲 >=0円 <=10000円\n  区分(b) : 真偽\n\n\
               出力\n  r(r) : 真偽\n\n定義 大口(bulk) : 真偽 = 金額 >= 3000円\n\n\
               表 x(x)\n方式 一意\n| 大口 | 区分 | → r(r) : 真偽 |\n\
               | 真   | -    | 真 |\n| -    | 真   | 偽 |\n| 偽   | 偽   | 偽 |\n";
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
    // 同じ入力について `>=3万円` と `<=1000円` は同時に成り立たないが、
    // 領域解析は定義を自由軸として扱うのでこの交差を消せない。証人を
    // 構成できないので、`一意` でもエラーにせず W114 と番人へ降ろす
    // （未証明の存在で CI を止めない。§6.2）。**非存在の証明ではない**ので
    // 生成コードには実行時の番人が入る。
    let src = "規則 t(t) v1\n\n入力\n  金額(a) : 金額[円, 税込]  範囲 >=0円 <=10万円\n\n\
               出力\n  r(r) : 真偽\n\n\
               定義 大口(bulk) : 真偽 = 金額 >= 3万円\n定義 小口(small) : 真偽 = 金額 <= 1000円\n\n\
               表 x(x)\n方式 一意\n| 大口 | 小口 | → r(r) : 真偽 |\n\
               | 真   | -    | 真 |\n| -    | 真   | 偽 |\n| 偽   | 偽   | 偽 |\n";
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
    // 対になる番人が生成コードに入る（§8.1）。
    let (f, c) = rulec::prepare(src, "d.rule").expect("検査は通る");
    let py = rulec::codegen::Gen::new(&f, &c, src).python();
    assert!(py.contains("番人"), "番人が入っていない");
    assert!(py.contains("RuleContradictionError"), "番人が例外を投げない");
}

#[test]
fn readmeの例は通る() {
    // README に載せた例が腐らないように、毎回検査する。
    // 通らない例を README に載せるのは、この道具の趣旨に反する。
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let md = std::fs::read_to_string(&p).expect("README.md が読めない");
    let head = md.find("## 書き方").expect("## 書き方 の節が無い");
    let open = md[head..].find("```").expect("コードブロックが無い") + head + 3;
    let close = md[open..].find("```").expect("コードブロックが閉じていない") + open;
    let src = md[open..close].trim_start_matches('\n');

    let ds = rulec::check_source(src, "README.md");
    assert!(
        !rulec::has_error(&ds),
        "README の例が通らない: {:?}",
        ds.iter().map(|d| format!("{}: {}", d.code, d.title)).collect::<Vec<_>>()
    );

    // 「生成されるコード」に貼った抜粋が、実際の生成物と食い違わないこと。
    // 抜粋は `...` で端折ってあるので全体は比べられないが、載せた行が
    // 一行残らず生成物に在ることは確かめられる。README の中の生成物は
    // 手で書き写したもので、手で書き写したものは腐る。
    let (f, c) = rulec::prepare(src, "README.md").expect("README の例は検査を通る");
    let g = rulec::codegen::Gen::new(&f, &c, src);
    let py = g.python();
    let go = g.go();
    let sec = md.find("## 生成されるコード").expect("## 生成されるコード の節が無い");
    let end = md[sec..].find("## 何を検査するか").expect("節の終わりが無い") + sec;
    for (lang, body) in [("python", &py), ("go", &go)] {
        let fence = format!("```{lang}\n");
        let open = md[sec..end].find(&fence).unwrap_or_else(|| panic!("{lang} の抜粋が無い")) + sec + fence.len();
        let close = md[open..end].find("```").expect("抜粋が閉じていない") + open;
        for line in md[open..close].lines() {
            if line.trim() == "..." || line.trim().is_empty() {
                continue;
            }
            assert!(body.contains(line), "README の {lang} 抜粋が生成物に無い:\n{line}");
        }
    }
}

#[test]
fn 解析できない型の列は黙って飛ばさない() {
    // 型が解析できないと `TableRegion::build` が諦め、その表の完全性も重複も
    // 検査されないまま ok が出ていた。日付と optional で二度踏んだ形なので、
    // 一般の防波堤として E110 で止める（§6.3）。
    let src = "規則 t(t) v1\n\n入力\n  s(s) : 文字列\n\n出力\n  r(r) : 真偽\n\n\
               表 x(x)\n方式 一意\n| s | → r(r) : 真偽 |\n| \"a\" | 真 |\n";
    let ds = rulec::check_source(src, "s.rule");
    assert!(ds.iter().any(|d| d.code == "E110"), "解析できない列は E110: {:?}",
            ds.iter().map(|d| d.code).collect::<Vec<_>>());
}

#[test]
fn optionalの列も検査される() {
    // `T?` は「無し」を一つ足した列挙として扱う。穴を開ければ E101 が出る。
    let base = "規則 t(t) v1\n\n型 区分(k) = 甲(a) | 乙(b)\n\n\
                入力\n  金額(amt) : 金額[円, 税込]  範囲 >=0円 <=100万円\n  任意値(opt) : 区分?\n\n\
                出力\n  r(r) : 真偽\n\n表 x(x)\n方式 一意\n\
                | 金額      | 任意値   | → r(r) : 真偽 |\n\
                | <1000円   | 無し     | 真 |\n\
                | <1000円   | 甲 ・ 乙 | 偽 |\n\
                | >=1000円  | -        | 偽 |\n";
    let ds = rulec::check_source(base, "o.rule");
    assert!(!rulec::has_error(&ds), "完全な表は通る: {:?}",
            ds.iter().map(|d| format!("{}:{}", d.code, d.title)).collect::<Vec<_>>());

    // 「無し」の行を落とすと穴が開く。飛ばしていたら気づけない。
    let holed = base.replace("| <1000円   | 無し     | 真 |\n", "");
    let ds = rulec::check_source(&holed, "o.rule");
    assert!(ds.iter().any(|d| d.code == "E101"), "無し の穴を捕まえるはず: {:?}",
            ds.iter().map(|d| d.code).collect::<Vec<_>>());
}

/// README のキーワード表が、パーサが実際に受理する語と一致すること。
/// 語を足したのに表に書き忘れる、表に無い語を書く、のどちらも赤にする。
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
    // `規則` はヘッダ専用でパーサの一覧には無いので、こちらで足して比べる。
    let mut want: Vec<String> =
        rulec::parse::KEYWORDS.iter().map(|s| s.to_string()).chain(["規則".to_string()]).collect();
    want.sort();
    assert_eq!(listed, want, "README のキーワード表とパーサが食い違う");
}

/// `| `語` | 説明 |` の最初のセルだけを見るための小道具。
trait FirstCell {
    fn next_or_all(self) -> Vec<String>;
}
impl<'a, I: Iterator<Item = &'a str>> FirstCell for I {
    fn next_or_all(mut self) -> Vec<String> {
        let _ = self.next(); // 行頭の `|` の前の空文字
        self.next().map(|s| vec![s.to_string()]).unwrap_or_default()
    }
}
