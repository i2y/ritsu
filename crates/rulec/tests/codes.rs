//! The ledger of diagnostic codes (§11) is the single source.
//!
//! Three properties are pinned, and together they close the way a ledger normally rots:
//! a code the checker emits but the ledger has forgotten, an example that no longer
//! produces its code, and a checked-in `docs/codes*.md` that has drifted from the tool.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::TempDir;
use rulec::i18n::Lang;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ledger_codes() -> BTreeSet<&'static str> {
    rulec::codes::ledger().iter().map(|e| e.code).collect()
}

/// Every code any file under `dir` produces.
fn codes_seen_in(dir: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let d = root().join(dir);
    for e in std::fs::read_dir(&d).unwrap_or_else(|_| panic!("読めない: {dir}")).flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "rule") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        let rel = format!("{dir}/{}", p.file_name().unwrap().to_string_lossy());
        for d in rulec::check_source(&src, &rel) {
            out.insert(d.code.to_string());
        }
    }
    out
}

#[test]
fn 出しうるコードは全部台帳にある() {
    let have = ledger_codes();
    let mut seen = codes_seen_in("tests/mutants");
    seen.extend(codes_seen_in("tests/corpus"));
    // The golden snapshots are named after the code they pin, so they are a second list of
    // codes the tool really prints.
    for e in std::fs::read_dir(root().join("tests/golden")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if let Some(stem) = n.strip_suffix(".txt") {
            seen.insert(stem.split('-').next().unwrap().to_string());
        }
    }
    let missing: Vec<&String> = seen.iter().filter(|c| !have.contains(c.as_str())).collect();
    assert!(missing.is_empty(), "台帳に無いコードが出ている: {missing:?}");
}

/// What a reproduction prints, as (code, line), when it is run where its files really are:
/// an example that needs a file beside it (the `.proto` imports, a callee, a cited copy) is run
/// with the file there, so the companion is held to the same standard as the example.
fn printed(e: &rulec::codes::Entry, r: rulec::codes::Repro) -> Vec<(&'static str, usize)> {
    let budget = e.budget.unwrap_or(rulec::region::DEFAULT_BUDGET);
    let tmp = (!r.files.is_empty()).then(|| TempDir::new(&format!("explain-{}", e.code)));
    let dir = tmp.as_ref().map_or_else(PathBuf::new, |t| {
        let d = t.path().to_path_buf();
        for (name, text) in r.files {
            // A copy of a source lives under `sources/law/…`, so the directories come first.
            let p = d.join(name);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).expect("隣のディレクトリを作れない");
            }
            std::fs::write(&p, text).expect("隣のファイルを書けない");
        }
        d
    });
    let path = dir.join("explain.rule").to_string_lossy().into_owned();
    // koyomi is joined as `ritsu rulec` joins it, but for E129, which is what a rulec with
    // no koyomi says (§15.174).
    let port: Option<rulec::days::Port> = (e.code != "E129").then(|| std::sync::Arc::new(koyomi::ports::Engine) as rulec::days::Port);
    let ds = rulec::days::with(port, || rulec::report_with(r.example, &path, budget).diags);
    ds.iter()
        .map(|d| {
            let line = d.where_.strip_prefix(&format!("{path}:")).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok());
            (d.code, line.unwrap_or(0))
        })
        .collect()
}

/// The reproduction shown in English and the one shown in Japanese: each of them has to print
/// the code. An example that has rotted is worse than none: the prose around it still reads well.
#[test]
fn 台帳の例は本当にそのコードを出す() {
    for e in rulec::codes::ledger() {
        for lang in [Lang::En, Lang::Ja] {
            // the reproduction, and the one of each other form the entry shows (E102, §15.189)
            let all: Vec<rulec::codes::Repro> = std::iter::once(e.repro(lang)).chain(e.also.iter().map(|a| a.repro(lang))).collect();
            for r in all {
                let got = printed(&e, r);
                assert!(
                    got.iter().any(|(c, _)| *c == e.code),
                    "{} の{}の例が {} を出さない（出たのは {:?}）:\n{}",
                    e.code,
                    if lang == Lang::En { "英語" } else { "日本語" },
                    e.code,
                    got,
                    r.example
                );
            }
        }
    }
}

/// The other forms an entry reproduces are those forms, not the first one again: E102's are a row
/// past what a derive reaches (§15.189), past what a `define` reaches, past what a derive reaches
/// while a table above writes the value the row asks for (§15.195), and at the truth value a
/// boolean `define` never takes (§15.196), and each note says what the value comes to, in both
/// languages. Each English twin says the same things in the same places
/// as the Japanese one.
#[test]
fn 台帳の別の形の再現はその形を出す() {
    let e = rulec::codes::find("E102").unwrap();
    assert_eq!(e.also.len(), 4);
    let says: [[(Lang, &str); 2]; 4] = [
        [(Lang::En, "can only come to >=-99JPY <=100JPY"), (Lang::Ja, "が取りうる値は、入力の範囲から計算すると >=-99円 <=100円 です。")],
        [(Lang::En, "`share` can only come to >=0% <=100%"), (Lang::Ja, "`割合` が取りうる値は、入力の範囲から計算すると >=0% <=100% です。")],
        [(Lang::En, "`band` is small only where row 1 of table k (amount <= 50JPY) fires. There `excess` can only come to >=-99JPY <=50JPY"), (Lang::Ja, "`区分` が 少額 になるのは、表 k の 行1（額 <= 50円）が当たるときだけです。そのとき `超過` が取りうる値は >=-99円 <=50円 で")],
        [(Lang::En, "Over the ranges of the inputs, `in_period` is always true, and this row asks for it to be false."), (Lang::Ja, "入力の範囲では、`期間内` はいつも true です。この行は false を求めています。")],
    ];
    for (a, said) in e.also.iter().zip(says) {
        for (lang, said) in said {
            let r = a.repro(lang);
            let notes = rulec::i18n::with(lang, || rulec::check_source(r.example, "explain.rule").iter().filter(|d| d.code == "E102").flat_map(|d| d.notes.clone()).collect::<Vec<_>>().join("\n"));
            assert!(notes.contains(said), "{lang:?}:\n{notes}");
        }
        assert_eq!(printed(&e, a.repro(Lang::En)), printed(&e, a.repro(Lang::Ja)));
    }
    // the entries that show another form: E102; E031 with an output of one element (§15.208);
    // E022 with a `max` that has no `empty` (§15.219); E108 with the points of the random inputs
    // (§15.224); and, with the rule beside them that a contribution adds to or an apply reads, E033
    // with a value a contribution adds, E040 with a contribution's signature, and E044 with applies
    // that go round (§15.228, §15.230); E103 with a table's heading that writes a type other than
    // the output's (§15.247), and with `none` under an output that is not optional (§15.249); E043
    // with a value outside the range of an input of an ordered enum (§15.252), and with a value that
    // may be absent (§15.255); E121 with a path that goes on through a collection (§15.254); E034
    // with an enum's value written twice, two names of one identifier, a column twice in a table's
    // heading, and the label of two rows (§15.262), and two enums whose types take one name; W121 with
    // two names Swift and Java spell alike, two SQLite reads as one column, two PostgreSQL cuts to
    // one, and a name Ruby reads as a constant (§15.263), a name the generated code uses itself and a
    // name of `_` alone (§15.265); E011 with a sequence that has no ASCII alias (§15.265); E140
    // with an output's range that cannot be checked (§15.268); E076 with each other way a rule and
    // the contract it implements part (§15.274)
    let with: Vec<&str> = rulec::codes::ledger().iter().filter(|e| !e.also.is_empty()).map(|e| e.code).collect();
    assert_eq!(with, ["E011", "E022", "E031", "E033", "E034", "E040", "E043", "E044", "E076", "E102", "E103", "E108", "E121", "E140", "W121"]);
    // Each by its place among the entry's other forms; `None` is the last.
    for (code, form, says) in [
        ("E033", None, [(Lang::En, "The added value z has neither a row of table t nor `default`"), (Lang::Ja, "足した値 z に、表 t の行も `default` もありません")]),
        ("E040", None, [(Lang::En, "The shape of `base.rule` this contribution relies on has changed"), (Lang::Ja, "この寄与が頼る `base.rule` の形が変わっています")]),
        ("E043", Some(0), [(Lang::En, "A value passed to `campaign` leaves the callee's range"), (Lang::Ja, "`campaign` に渡す値が、元の規則の範囲の外に出ます")]),
        ("E043", None, [(Lang::En, "What is passed to `t` may be absent, and the callee's `t` takes no absent value"), (Lang::Ja, "`t` に渡す値には値の無いことがありますが、元の規則の `t` は値の無いことを受け付けません")]),
        ("E044", None, [(Lang::En, "the applies go round"), (Lang::Ja, "準用が輪になっています")]),
        ("E121", None, [(Lang::En, "The path `order.lines.category` goes on through the collection `order.lines`"), (Lang::Ja, "パス `order.lines.category` は、並び `order.lines` の先へ続いています")]),
        ("E103", Some(0), [(Lang::En, "Output r is money[JPY, excl_tax], but money[JPY, incl_tax] is written here"), (Lang::Ja, "出力 r は money[円, excl_tax] ですが、ここには money[円, incl_tax] と書かれています")]),
        ("E103", Some(1), [(Lang::En, "This column is money[JPY, incl_tax], but `none` is written here"), (Lang::Ja, "この列は money[円, incl_tax] ですが `none` が書かれています")]),
        ("E034", Some(0), [(Lang::En, "Enum `member_kind` has the value `basic` twice"), (Lang::Ja, "列挙 `会員区分` の値 `一般` が二度書かれています")]),
        ("E034", Some(1), [(Lang::En, "`weight` and `width` become the one identifier `w` in the generated code"), (Lang::Ja, "`重さ` と `長さ` が、生成するコードで同じ識別子 `weight` になります")]),
        ("E034", Some(2), [(Lang::En, "The heading has the column `weight` twice"), (Lang::Ja, "見出しに列 `重さ` が二度あります")]),
        ("E034", Some(3), [(Lang::En, "The types of enums `tier_level` and `tierLevel` take the one name `TierLevel`"), (Lang::Ja, "列挙 `会員の段階` と `地域の段階` の型が、生成するコードで同じ名前 `TierLevel` になります")]),
        ("W121", Some(0), [(Lang::En, "`base_fee` and `baseFee` are one identifier in the code generated for Swift and Java"), (Lang::Ja, "`基本料金` と `追加料金` は、Swift・Java の生成コードで同じ識別子になります")]),
        ("W121", Some(1), [(Lang::En, "`afee` and `aFee` are one identifier in the code generated for SQL (SQLite)"), (Lang::Ja, "`基本料金` と `追加料金` は、SQL（SQLite）の生成コードで同じ識別子になります")]),
        ("W121", Some(2), [(Lang::En, "are one identifier in the code generated for SQL (PostgreSQL)"), (Lang::Ja, "は、SQL（PostgreSQL）の生成コードで同じ識別子になります")]),
        ("W121", Some(3), [(Lang::En, "`Base` starts with a capital letter, which the code generated for Ruby cannot take"), (Lang::Ja, "`Base` は大文字で始まるので、Ruby の生成コードでは使えません")]),
        ("W121", Some(4), [(Lang::En, "`trace` takes a name the code generated for Connect uses itself"), (Lang::Ja, "`記録` は、Connect の生成コードが自分で使う名前と同じになります")]),
        ("W121", None, [(Lang::En, "`_` is a name the code generated for Rust, Go, Swift, Java and Wasm cannot take"), (Lang::Ja, "`_` は、Rust・Go・Swift・Java・Wasm の生成コードでは使えません")]),
        ("E011", None, [(Lang::En, "A public name has no ASCII alias"), (Lang::Ja, "公開される名前に ASCII の別名がありません")]),
        ("E034", None, [(Lang::En, "The row label `r1` appears twice"), (Lang::Ja, "行ラベル `r1` が二度あります")]),
        ("E140", None, [(Lang::En, "The range of the output fee cannot be checked"), (Lang::Ja, "出力 手数料 の範囲を確かめられません")]),
        ("E076", Some(0), [(Lang::En, "The contract's `QuoteRequest` has `express`, which the rule does not"), (Lang::Ja, "契約の `QuoteRequest` の `express` が、規則にありません")]),
        ("E076", Some(1), [(Lang::En, "`subtotal` has another type in the contract"), (Lang::Ja, "`subtotal` の型が、契約と違います")]),
        ("E076", Some(2), [(Lang::En, "Whether `subtotal` is `optional` does not agree with the contract"), (Lang::Ja, "`subtotal` の `optional` が、契約と合っていません")]),
        ("E076", Some(3), [(Lang::En, "The enum `tier` is not read from a contract"), (Lang::Ja, "列挙 `tier` は、契約から読んだものではありません")]),
        ("E076", Some(4), [(Lang::En, "The contract's `QuoteResponse` has no `trace`"), (Lang::Ja, "契約の `QuoteResponse` に `trace` がありません")]),
        ("E076", Some(5), [(Lang::En, "The contract's service `ShippingService` is not shaped as a rule's service"), (Lang::Ja, "契約のサービス `ShippingService` は、規則のサービスの形ではありません")]),
        ("E076", Some(6), [(Lang::En, "The contract's `QuoteRequest` writes `subtotal` in a way a rule's service does not"), (Lang::Ja, "契約の `QuoteRequest` の `subtotal` は、規則のサービスが読まない書き方です")]),
        ("E076", None, [(Lang::En, "The contract `shipping.proto` is not proto3"), (Lang::Ja, "契約 `shipping.proto` は proto3 ではありません")]),
    ] {
        let e = rulec::codes::find(code).unwrap();
        let a = match form {
            Some(i) => &e.also[i],
            None => e.also.last().unwrap(),
        };
        for (lang, said) in says {
            let r = a.repro(lang);
            let tmp = TempDir::new(&format!("also-{code}"));
            for (name, text) in r.files {
                std::fs::write(tmp.path().join(name), text).unwrap();
            }
            let path = tmp.path().join("explain.rule").to_string_lossy().into_owned();
            let titles = rulec::i18n::with(lang, || rulec::check_source(r.example, &path).iter().filter(|d| d.code == code).map(|d| d.title.clone()).collect::<Vec<_>>().join("\n"));
            assert!(titles.contains(said), "{code} {lang:?}:\n{titles}");
        }
    }
    // `explain` shows each under a heading of its own, in the language asked for, and the JSON
    // under `also`, a key no other entry has
    let (_, en) = run(&["explain", "E102", "--lang", "en"]);
    assert!(en.contains("Smallest reproduction of the third form") && en.contains("amount(a) : money[JPY, incl_tax]"), "{en}");
    assert!(en.contains("a row outside what a `define` can reach") && en.contains("define share(s) : rate = score / 200"), "{en}");
    let (_, ja) = run(&["explain", "E102", "--lang", "ja"]);
    assert!(ja.contains("三つ目の形") && ja.contains("額(a)   : money[円, incl_tax]"), "{ja}");
    assert!(ja.contains("`define` の取りうる値の外にある行") && ja.contains("define 割合(s) : rate = 点 / 200"), "{ja}");
    let (_, j) = run(&["explain", "E102", "--format", "json", "--lang", "en"]);
    let j = rulec::json::parse(&j).unwrap();
    assert!(j.get("also").and_then(|a| a.as_arr()).is_some_and(|a| a.len() == 4), "{j:?}");
    let (_, j) = run(&["explain", "E101", "--format", "json", "--lang", "en"]);
    assert!(rulec::json::parse(&j).unwrap().get("also").is_none());
}

/// An English twin is the same rule with other names, so it has to say the same things in the
/// same places: the same codes on the same lines as the Japanese one. A twin that drifts (a name
/// that collides with a keyword, a unit that is read another way) shows up here, not in a reader.
#[test]
fn 英語の再現は日本語の再現と同じ診断を同じ行に出す() {
    let mut twins = 0;
    for e in rulec::codes::ledger() {
        let Some(en) = e.english else { continue };
        twins += 1;
        assert_eq!(printed(&e, en), printed(&e, e.repro(Lang::Ja)), "{} の英語の例が日本語の例と違う診断を出す", e.code);
    }
    // The twins are the reproductions that had Japanese names, strings or units: 61 of the 112
    // entries, less the six that stay as they are (below); and E013, whose Japanese
    // reproduction misspells `std/都道府県` and whose English one misspells `std/us/states`
    // (DESIGN §15.182); and W901, written with both from the start (ritsu's DESIGN 16.3). E075 and
    // W128 came with their twins (§15.208, §15.209), and so did E066 and E069 (§15.210, §15.212) and
    // E132 and E139 (§15.213, §15.215), and E074 (§15.223), and E140 (§15.268).
    assert_eq!(twins, 65, "英語の再現を持つ項目の数が変わった: {twins}");
}

/// Japanese is left in a reproduction shown in English only where being Japanese is the point of
/// the entry, and the list says which and why. Any other entry shows English names and units, so
/// a reproduction written with Japanese ones for a new code has to come with its English twin.
#[test]
fn 英語の再現に日本語が残るのは決めた項目だけ() {
    const KEPT: &[(&str, &str)] = &[
        ("E011", "the point is a name that is not ASCII, and its prose shows 重量(weight)"),
        ("E037", "cites a Japanese statute through e-Gov: the article 第1条 and the copy of its XML"),
        ("E038", "the same, with a pinned fragment that has changed"),
        ("E039", "the same, with no copy of the article"),
        ("W119", "the same, with a pin that nothing cites"),
    ];
    let japanese = |s: &str| {
        s.chars().any(|c| matches!(c, '\u{3000}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{ff00}'..='\u{ffef}'))
    };
    for e in rulec::codes::ledger() {
        let r = e.repro(Lang::En);
        let has = japanese(r.example) || r.files.iter().any(|(n, t)| japanese(n) || japanese(t));
        let kept = KEPT.iter().any(|(c, _)| *c == e.code);
        assert_eq!(has, kept, "{}: 英語の再現に日本語が{}", e.code, if has { "残っている（残すなら KEPT に理由を書く）" } else { "無い（KEPT から外す）" });
        // the reproduction of another form has no reason to keep any
        for a in &e.also {
            assert!(!japanese(a.repro(Lang::En).example), "{}: 別の形の英語の再現に日本語が残っている", e.code);
        }
    }
}

#[test]
fn 台帳は重複せず_関係するコードも台帳にある() {
    let all = rulec::codes::ledger();
    let mut seen = BTreeSet::new();
    for e in &all {
        assert!(seen.insert(e.code), "台帳に {} が二度ある", e.code);
        assert!(!e.title.is_empty() && !e.when.is_empty() && !e.fix.is_empty(), "{} のフィールドが空", e.code);
        for r in e.related {
            assert!(all.iter().any(|x| x.code == *r), "{} が台帳に無い {r} を指している", e.code);
            assert_ne!(r, &e.code, "{} が自分自身を指している", e.code);
        }
    }
    // Every code that has a golden snapshot, and every code in the DESIGN ledger, is here;
    // `出しうるコードは全部台帳にある` covers the first. There are no vacant numbers left.
    // W901 (ritsu's DESIGN 16.3) made it 113, E075 and W128 (§15.208, §15.209) 115, E066 and E069
    // (§15.210, §15.212) 117, E132 and E139 (§15.213, §15.215) 119, E072, E134 and E135
    // (§15.217) 122, E071, E136, E137, E138 and W130 (§15.222) 127, E074 (§15.223) 128, E070,
    // E073, E133, W129 and W131 (§15.224–§15.226) 133, E067, E068, E131 and W132 (§15.227)
    // 137, E140 (§15.268) 138, and E076 (§15.274) 139.
    assert_eq!(all.len(), 139, "台帳の件数が変わった: {}", all.len());
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// `docs/codes*.md` is a generated file: checked in, and held to the current output.
/// Re-bake with `cargo run -- explain --all --format markdown --lang en > docs/codes.md`
/// (and `--lang ja > docs/codes.ja.md`).
#[test]
fn チェックインされた文書はいまの出力と同じ() {
    for (lang, file) in [("en", "docs/codes.md"), ("ja", "docs/codes.ja.md")] {
        let (c, got) = run(&["explain", "--all", "--format", "markdown", "--lang", lang]);
        assert_eq!(c, 0);
        let p = root().join(file);
        assert!(Path::new(&p).exists(), "{file} がありません。生成して入れてください");
        let want = std::fs::read_to_string(&p).unwrap();
        assert_eq!(
            want, got,
            "{file} が古いか手で編集されています。\
             `cargo run -- explain --all --format markdown --lang {lang} > {file}` で焼き直してください"
        );
    }
}

/// `explain` shows the reproduction of the language it is asked in: the English twin in English
/// (names, units and the files beside it), the Japanese one under `--lang ja` as it was, and the
/// same one in both for an entry whose point is Japanese.
#[test]
fn explainは言語ごとの再現を見せる() {
    let repro_of = |code: &str, lang: &str| -> String {
        let (c, out) = run(&["explain", code, "--lang", lang]);
        assert_eq!(c, 0);
        let heading = if lang == "en" { "Smallest reproduction" } else { "最小の再現" };
        out.split(heading).nth(1).unwrap_or_else(|| panic!("{code} に再現の見出しが無い:\n{out}")).to_string()
    };
    // A rule with a money unit.
    let (en, ja) = (repro_of("E014", "en"), repro_of("E014", "ja"));
    assert!(en.contains("round down(1JPY)") && !en.contains('円'), "{en}");
    assert!(ja.contains("round down(1円)") && !ja.contains("JPY"), "{ja}");
    // A callee beside it: its name and what is in it.
    let (en, ja) = (repro_of("E043", "en"), repro_of("E043", "ja"));
    assert!(en.contains("callee.rule") && en.contains("small |") && !en.contains("呼"), "{en}");
    assert!(ja.contains("呼び先.rule") && ja.contains("小 |") && !ja.contains("small"), "{ja}");
    // One that stays Japanese in both.
    let (en, ja) = (repro_of("E011", "en"), repro_of("E011", "ja"));
    assert!(en.contains("重量 : bool") && ja.contains("重量 : bool"));
    // The JSON follows the language in `example` and `files`, with the same keys.
    let line = |lang: &str| run(&["explain", "E043", "--format", "json", "--lang", lang]).1;
    let (en, ja) = (line("en"), line("ja"));
    assert!(en.contains("\"name\":\"callee.rule\"") && en.contains("round down(1)"), "{en}");
    assert!(ja.contains("\"name\":\"呼び先.rule\"") && ja.contains("round down(1)"), "{ja}");
}

#[test]
fn 知らないコードは2で止まる() {
    let (c, _) = run(&["explain", "E999"]);
    assert_eq!(c, 2);
    let (c, out) = run(&["explain", "e101"]);
    assert_eq!(c, 0, "コードの大小文字は問わない");
    assert!(out.contains("E101"), "{out}");
}

#[test]
fn jsonは鍵が英語で読み戻せる() {
    let (c, out) = run(&["explain", "--all", "--format", "json", "--lang", "ja"]);
    assert_eq!(c, 0);
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), rulec::codes::ledger().len(), "一行一件でない");
    for l in &lines {
        let j = rulec::json::parse(l).unwrap_or_else(|e| panic!("JSON として読めない: {e}\n{l}"));
        for k in ["code", "severity", "title", "when", "fix", "example", "related", "lang"] {
            assert!(j.get(k).is_some(), "{k} が無い: {l}");
        }
        assert_eq!(j.get("lang").unwrap().as_str(), Some("ja"));
    }
    // The keys are the stable API; only the prose follows `--lang`.
    let (_, en) = run(&["explain", "--all", "--format", "json", "--lang", "en"]);
    let key_of = |s: &str| -> Vec<String> {
        s.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| rulec::json::parse(l).unwrap().get("code").unwrap().as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(key_of(&out), key_of(&en));
}

/// The codes a command's `--help` names must be codes the ledger carries. Without this the
/// CLI table and the ledger drift apart, which is the exact failure this step removes.
#[test]
fn helpが挙げるコードは台帳にある() {
    let have = ledger_codes();
    let (_, help) = run(&["--help", "--lang", "en"]);
    let cmds: Vec<String> = help
        .lines()
        .filter_map(|l| l.strip_prefix("  rulec "))
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect();
    let mut named = 0usize;
    for c in &cmds {
        let (_, page) = run(&[c, "--help", "--lang", "en"]);
        let Some(tail) = page.split("Diagnostics it can print").nth(1).and_then(|t| t.split(":\n").nth(1))
        else {
            continue;
        };
        for code in tail.split_whitespace() {
            assert!(have.contains(code), "`rulec {c} --help` が台帳に無い {code} を挙げている");
            named += 1;
        }
    }
    assert!(named >= 30, "どの help もコードを挙げていない");
}

/// And the other way round: every code in the ledger is on `check`'s page, and on `gen`'s,
/// which prints check's findings when it refuses to generate. A list kept by hand falls
/// behind the ledger, so the pages take theirs from it.
#[test]
fn 台帳のコードは全部checkとgenのhelpにある() {
    for c in ["check", "gen"] {
        let (_, page) = run(&[c, "--help", "--lang", "en"]);
        let tail = page.split("Diagnostics it can print").nth(1).and_then(|t| t.split(":\n").nth(1)).unwrap_or("");
        let named: Vec<&str> = tail.split_whitespace().collect();
        for code in ledger_codes() {
            assert!(named.contains(&code), "`rulec {c} --help` に {code} が無い");
        }
    }
}

// ── Two English sentences that used to carry something Japanese or out of order ────────────

/// E103's note on a rounding grid that is not a value of its column gives its example in the unit
/// of the column (`1JPY` for yen, `1g` for grams, `1円` on a column written in 円), not a yen
/// amount whatever the column holds. The Japanese sentence is as it was.
#[test]
fn 丸めの刻みの英語の例は列の単位で書かれる() {
    let note = |lang: Lang, ty: &str, range: &str| -> String {
        let src = format!("rule t(t) v1\n\ninputs\n  p(p) : {ty}  range {range}\n\noutputs\n  r(r) : {ty}  round down(1)\n\nresult r = p\n");
        rulec::i18n::with(lang, || {
            let ds = rulec::check_source(&src, "t.rule");
            let d = ds.iter().find(|d| d.code == "E103").unwrap_or_else(|| panic!("E103 が出ない: {ty}"));
            d.notes.join("\n")
        })
    };
    let en = note(Lang::En, "money[JPY]", ">=0JPY <=10JPY");
    assert!(en.contains("(`money[JPY]` takes `round down(1JPY)`)"), "{en}");
    let en = note(Lang::En, "mass[g]", ">=0g <=10g");
    assert!(en.contains("(`mass[g]` takes `round down(1g)`)"), "{en}");
    let en = note(Lang::En, "money[円]", ">=0円 <=10円");
    assert!(en.contains("(`money[円]` takes `round down(1円)`)"), "{en}");
    let ja = note(Lang::Ja, "money[円]", ">=0円 <=10円");
    assert!(ja.contains("（`money[円]` なら `round down(1円)`）"), "{ja}");
}

/// E043's English example names the range and then the callee's file, as the Japanese one does
/// (the file and the range were swapped).
#[test]
fn e043の英語の例は範囲とファイルを取り違えない() {
    let dir = TempDir::new("e043-order");
    let callee = "rule callee(callee) v1\n\ninputs\n  years(years) : number  range >=1 <=40\n\noutputs\n  x(x) : number  round down(1)\n\ntable t(t)\npolicy unique\n| years | -> x |\n| -     | 1    |\n";
    std::fs::write(dir.path().join("callee.rule"), callee).unwrap();
    let main = "rule t(t) v1\n\ninputs\n  n(n) : number  range >=0 <=10\n\noutputs\n  y(y) : number  round down(1)\n\napply call(c) = \"callee.rule\"\n  years = n\n  x -> y\n";
    let path = dir.path().join("main.rule").to_string_lossy().into_owned();
    let note = |lang: Lang| -> String {
        rulec::i18n::with(lang, || {
            let ds = rulec::check_source(main, &path);
            let d = ds.iter().find(|d| d.code == "E043").unwrap_or_else(|| panic!("E043 が出ない: {ds:?}"));
            d.notes.join("\n")
        })
    };
    let en = note(Lang::En);
    assert!(en.contains("For example years = 0 is outside range >=1 <=40 of `years` in `callee.rule`."), "{en}");
    let ja = note(Lang::Ja);
    assert!(ja.contains("例: years = 0 は、`callee.rule` の `years` の range >=1 <=40 の外です。"), "{ja}");
}

// ── The reference (docs/reference.md) ────────────────────────────────────

/// The reserved-word table in the reference must be exactly `kw::RESERVED`. Both a word the
/// parser reserves but the document omits, and a word the document claims but the parser
/// does not reserve, go red — the same shape as the README's keyword test.
#[test]
fn 参照文書の予約語表はkwと一致する() {
    let doc = std::fs::read_to_string(root().join("docs/reference.md")).unwrap();
    let i = doc.find("<!-- RESERVED -->").expect("予約語表の印が無い");
    let j = doc.find("<!-- /RESERVED -->").expect("予約語表の閉じ印が無い");
    let mut listed: Vec<String> = Vec::new();
    for w in doc[i..j].split('`').skip(1).step_by(2) {
        listed.push(w.to_string());
    }
    listed.sort();
    listed.dedup();
    let mut want: Vec<String> = rulec::kw::RESERVED.iter().map(|s| s.to_string()).collect();
    want.sort();
    want.dedup();
    assert_eq!(listed, want, "docs/reference.md の予約語表と src/kw.rs が食い違う");
}

/// Every rule of the corpus has to be describable by the reference, so the reference has to
/// name every section word the corpus actually uses.
#[test]
fn 参照文書は行頭の語を全部説明している() {
    let doc = std::fs::read_to_string(root().join("docs/reference.md")).unwrap();
    for w in rulec::kw::LINE_HEAD.iter().chain([&rulec::kw::RULE]) {
        assert!(doc.contains(&format!("`{w}`")), "docs/reference.md に `{w}` の説明が無い");
    }
}
