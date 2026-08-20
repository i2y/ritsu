//! 過去再生（§10.2〜§10.4）。M3 の受け入れ。
//!
//! 実データは要らない。ベクタから合成 fixtures を作り、欠陥を仕込んだ版と
//! 突き合わせれば、実測系と補完系の分離、発火行クラスタ、丸め差異タグ、
//! git タグ糖衣まで一通り行使できる。実データに残るのは切り分けの「有効性」だけで、
//! それは §16 第二の危うさとして最初から記録を持つ側に残る設計である。

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const RULE: &str = "tests/corpus/ゆうパック運賃.rule";

fn rulec_in(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn rulec(args: &[&str]) -> (i32, String, String) {
    rulec_in(&root(), args)
}

/// 生成した正準ベクタから合成 fixtures を作る。ベクタの JSON はこちらが
/// 決めた形（`{"in":…,"out":…,"trace":…`）なので、位置で切り出してよい。
fn fixtures_from_vectors(vectors: &str) -> String {
    let mut out = String::new();
    for (i, l) in vectors.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let ins = &l[l.find("\"in\":").expect("in が無い") + 5..l.find(",\"out\":").expect("out が無い")];
        let obs = &l[l.find("\"out\":").expect("out が無い") + 6..l.find(",\"trace\":").expect("trace が無い")];
        out.push_str(&format!(
            "{{\"ts\":\"2025-08-14T09:12:33+09:00\",\"tag\":\"order:{i}\",\"in\":{ins},\"observed\":{obs}}}\n"
        ));
    }
    out
}

/// 作業場を用意し、ベクタと合成 fixtures を置く。
fn setup(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-m3-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.to_string_lossy().to_string();
    let (c, _, e) = rulec(&["vectors", RULE, "--out", &out]);
    assert_eq!(c, 0, "{e}");
    let v = std::fs::read_to_string(dir.join("yupack_fee.jsonl")).expect("ベクタが無い");
    std::fs::write(dir.join("fx.jsonl"), fixtures_from_vectors(&v)).unwrap();
    dir
}

/// 内容で当てる書き換え。当たらなかったら黙って通さない。
fn tweak(src: &str, from: &str, to: &str, want: usize) -> String {
    let n = src.matches(from).count();
    assert_eq!(n, want, "書き換えが当たっていない: `{from}` が {n} 箇所");
    src.replace(from, to)
}

#[test]
fn 忠実な記録とは完全に一致する() {
    let dir = setup("clean");
    let fx = dir.join("fx.jsonl");
    let (c, out, e) = rulec(&["replay", RULE, "--fixtures", fx.to_str().unwrap()]);
    assert_eq!(c, 0, "{out}{e}");
    assert!(out.contains("(100.000%)"), "{out}");
    assert!(out.contains("不一致はありません"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.2: 型と範囲の検証だけを引き受ける。壊れた記録は**捨てずに報告する**。
/// 黙って落とすと、分母が縮んだ分だけ一致率が上がって見える。
#[test]
fn 汚れた記録は種類ごとに数えて報告する() {
    let dir = setup("lint");
    let mut lines: Vec<String> =
        std::fs::read_to_string(dir.join("fx.jsonl")).unwrap().lines().map(String::from).collect();
    let bad = [
        r#"{"tag":"order:b1","in":{"あて先":"東京都""#,
        r#"{"tag":"order:b2","in":{"あて先":"東京都","三辺合計":50,"重量":1000,"配送業者":"ヤマト"},"observed":{"運賃":820}}"#,
        r#"{"tag":"order:b3","in":{"あて先":"江戸","三辺合計":50,"重量":1000},"observed":{"運賃":820}}"#,
        r#"{"tag":"order:b4","in":{"あて先":"東京都","三辺合計":900,"重量":1000},"observed":{"運賃":820}}"#,
        r#"{"tag":"order:b5","in":{"あて先":"東京都","三辺合計":50,"重量":1000},"observed":{}}"#,
        r#"{"tag":"order:b6","in":{"あて先":"東京都","三辺合計":50,"重量":1000},"observed":{"運賃":820.5}}"#,
    ];
    for b in bad {
        lines.push(b.into());
    }
    // 欄が欠けている記録（既定値が無ければ丸ごと外す）。
    for k in 0..5 {
        lines.push(format!(
            r#"{{"tag":"order:m{k}","in":{{"あて先":"東京都","三辺合計":50}},"observed":{{"運賃":820}}}}"#
        ));
    }
    let p = dir.join("dirty.jsonl");
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();

    let (c, out, _) = rulec(&["fixtures", "lint", p.to_str().unwrap(), RULE]);
    assert_eq!(c, 1, "壊れた記録があれば 1 で終わる: {out}");
    for want in [
        "JSON として読めません",
        "小数は受け付けません",
        "規則が知らない欄",
        "列挙 都道府県 の値ではありません",
        "宣言範囲 1..170 の外",
        "`observed.運賃` がありません",
    ] {
        assert!(out.contains(want), "`{want}` を言っていない:\n{out}");
    }
    assert!(out.contains("欄が欠けていたので外した記録: 5 件"), "{out}");
    // 証人（何行目の、どの記録か）が要る。
    assert!(out.contains("order:b3"), "証人を出す: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.3: 見出しの一致率は実測系だけから。補完は二次集計にしか効かない。
/// そして**使った既定値と欄ごとの補完件数をレポート自身が刻む**。
#[test]
fn 補完系は分けて数え_使った既定値を刻む() {
    let dir = setup("fill");
    let mut lines: Vec<String> =
        std::fs::read_to_string(dir.join("fx.jsonl")).unwrap().lines().map(String::from).collect();
    let base = lines.len();
    for k in 0..4 {
        lines.push(format!(
            r#"{{"tag":"order:m{k}","in":{{"あて先":"東京都","三辺合計":50}},"observed":{{"運賃":820}}}}"#
        ));
    }
    let p = dir.join("miss.jsonl");
    std::fs::write(&p, lines.join("\n") + "\n").unwrap();
    let fx = p.to_str().unwrap();

    // 既定値が無ければ、欄の欠けた記録は丸ごと外れる。
    let (_, plain, _) = rulec(&["replay", RULE, "--fixtures", fx]);
    assert!(plain.contains(&format!("照合 {base} 件")), "実測系だけを数える: {plain}");
    assert!(plain.contains("欄が欠けていて既定値も無い記録を 4 件外しました"), "{plain}");

    // マニフェストで補うと、補完系として別に数え、既定値が刻まれる。
    let m = dir.join("m.json");
    std::fs::write(&m, r#"{"rulec":"replay/1","rule":"ゆうパック運賃","fills":{"重量":1000}}"#).unwrap();
    let (_, filled, _) =
        rulec(&["replay", RULE, "--fixtures", fx, "--manifest", m.to_str().unwrap()]);
    assert!(filled.contains(&format!("照合 {base} 件")), "補完を見出しに混ぜている: {filled}");
    assert!(filled.contains("補完系 4 件（重量 4 件）"), "欄ごとの件数を刻む: {filled}");
    assert!(filled.contains("使った既定値: 重量 = 1000"), "既定値を刻む: {filled}");

    // --fill は感度分析の一時上書き。マニフェストより後に効く。
    let (_, over, _) = rulec(&["replay", RULE, "--fixtures", fx, "--manifest", m.to_str().unwrap(), "--fill", "重量=2000"]);
    assert!(over.contains("使った既定値: 重量 = 2000"), "上書きが効いていない: {over}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.4: 値だけが動いた版は、発火行が同じまま金額だけが動く。
#[test]
fn 値の変更は発火行を動かさない() {
    let dir = setup("value");
    let src = std::fs::read_to_string(root().join(RULE)).unwrap();
    let v2 = dir.join("v2.rule");
    std::fs::write(&v2, tweak(&src, "| 沖縄       | S60    | 1450円", "| 沖縄       | S60    | 1550円", 1)).unwrap();

    let (c, out, _) = rulec(&[
        "diff",
        RULE,
        v2.to_str().unwrap(),
        "--fixtures",
        dir.join("fx.jsonl").to_str().unwrap(),
    ]);
    assert_eq!(c, 1, "差があれば 1 で終わる: {out}");
    assert!(out.contains("影響 7 件"), "{out}");
    assert!(out.contains("金額 +700"), "動く金額を出す: {out}");
    assert!(out.contains("差 +100 一様"), "一様なら一行に畳む: {out}");
    assert!(!out.contains("→行"), "行は動いていないのに遷移を出している: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.4: 境界が動いた版は、発火行の遷移として出る。
#[test]
fn 境界の変更は発火行の遷移として出る() {
    let dir = setup("shift");
    let src = std::fs::read_to_string(root().join(RULE)).unwrap();
    let v3 = dir.join("v3.rule");
    std::fs::write(&v3, tweak(&src, "| <=60cm   | S60", "| <=50cm   | S60", 1)).unwrap();

    let (c, out, _) = rulec(&[
        "diff",
        RULE,
        v3.to_str().unwrap(),
        "--fixtures",
        dir.join("fx.jsonl").to_str().unwrap(),
    ]);
    assert_eq!(c, 1);
    assert!(out.contains("表 サイズ判定 行1→行2"), "遷移を出す: {out}");
    assert!(out.contains("表 運賃表 行1→行2"), "下流の遷移も出す: {out}");
    // 鍵は発火行の組だけ（§10.4）。旧の額が違っても、同じ遷移は同じクラスタ。
    let clusters = out.lines().filter(|l| l.starts_with("  表 ")).count();
    let rows = out.lines().filter(|l| l.contains("行1→行2")).count();
    assert!(rows >= 5 && clusters == rows, "遷移ごとに一クラスタのはず: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.4: 出力格子より小さいずれだけのクラスタには丸め差異の疑いが付く。
#[test]
fn 格子未満のずれは丸め差異として括られる() {
    let dir = setup("round");
    // 記録側を 3 円だけずらす。出力格子は 10円 なので、これは丸めの規約差の形。
    let src = std::fs::read_to_string(dir.join("fx.jsonl")).unwrap();
    let mut out = String::new();
    for l in src.lines() {
        if l.contains("\"あて先\":\"沖縄県\"") {
            let i = l.rfind("\"運賃\":").expect("運賃 が無い") + "\"運賃\":".len();
            let j = l[i..].find('}').unwrap() + i;
            let v: i64 = l[i..j].parse().unwrap();
            out.push_str(&format!("{}{}{}\n", &l[..i], v - 3, &l[j..]));
        } else {
            out.push_str(l);
            out.push('\n');
        }
    }
    let p = dir.join("round.jsonl");
    std::fs::write(&p, out).unwrap();

    let (c, r, _) = rulec(&["replay", RULE, "--fixtures", p.to_str().unwrap()]);
    assert_eq!(c, 1);
    assert!(r.contains("丸め差異の疑い（出力格子 10円 未満"), "{r}");
    assert!(r.contains("差 +3 一様"), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §1.4: `送料@v3` は git タグ `rules/送料/v3` を引く糖衣。
#[test]
fn 版の参照はgitタグを引く() {
    let Some(_) = which("git") else {
        eprintln!("注意: git が無いので飛ばした");
        return;
    };
    let dir = setup("git");
    let repo = dir.join("repo");
    std::fs::create_dir_all(repo.join("rules")).unwrap();
    let git = |args: &[&str]| {
        let o = Command::new("git").current_dir(&repo).args(args).output().expect("git");
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "-q", "."]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "t"]);

    let src = std::fs::read_to_string(root().join(RULE)).unwrap();
    let rel = "rules/ゆうパック運賃.rule";
    std::fs::write(repo.join(rel), &src).unwrap();
    git(&["add", rel]);
    git(&["commit", "-qm", "v1"]);
    git(&["tag", "rules/ゆうパック運賃/v1"]);

    std::fs::write(repo.join(rel), tweak(&src, "| 沖縄       | S60    | 1450円", "| 沖縄       | S60    | 1550円", 1)).unwrap();
    git(&["add", rel]);
    git(&["commit", "-qm", "v2"]);
    git(&["tag", "rules/ゆうパック運賃/v2"]);

    let fx = repo.join("fx.jsonl");
    std::fs::copy(dir.join("fx.jsonl"), &fx).unwrap();
    let (c, out, e) = rulec_in(
        &repo,
        &["diff", "ゆうパック運賃@v1", "ゆうパック運賃@v2", "--fixtures", "fx.jsonl"],
    );
    assert_eq!(c, 1, "{out}{e}");
    assert!(out.contains("ゆうパック運賃@v1 → ゆうパック運賃@v2"), "{out}");
    assert!(out.contains("影響 7 件"), "{out}");

    // 別の規則どうしを比べようとしたら止める。
    std::fs::write(repo.join("other.rule"), std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap()).unwrap();
    let (c, _, e) = rulec_in(&repo, &["diff", "ゆうパック運賃@v1", "other.rule", "--fixtures", "fx.jsonl"]);
    assert_eq!(c, 2, "別の規則を通している");
    assert!(e.contains("別の規則"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §12: PR に貼る形。整形まで道具が持ち、投稿は CI の一行に任せる。
#[test]
fn markdownで貼れる形が出る() {
    let dir = setup("md");
    let src = std::fs::read_to_string(root().join(RULE)).unwrap();
    let v2 = dir.join("v2.rule");
    std::fs::write(&v2, tweak(&src, "| 沖縄       | S60    | 1450円", "| 沖縄       | S60    | 1550円", 1)).unwrap();
    let (_, out, _) = rulec(&[
        "diff",
        RULE,
        v2.to_str().unwrap(),
        "--fixtures",
        dir.join("fx.jsonl").to_str().unwrap(),
        "--format",
        "markdown",
    ]);
    assert!(out.starts_with("### 規則 ゆうパック運賃 v1 — 版の差分"), "{out}");
    assert!(out.contains("| 照合（実測系） |"), "{out}");
    assert!(out.contains("#### 不一致の内訳"), "{out}");
    // 表がセル区切りで壊れないこと。
    for l in out.lines().filter(|l| l.starts_with("| 表 ")) {
        assert_eq!(l.matches(" | ").count(), 3, "列がずれている: {l}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// §9.3-3、§12: 生成物を利用者の CI でも回せること。
#[test]
fn rulec_testが生成物を走らせる() {
    if which("python3").is_none() && which("go").is_none() {
        eprintln!("注意: python3 も go も無いので飛ばした");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-m3-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let (c, _, e) = rulec(&["gen", RULE, "--out", &out]);
    assert_eq!(c, 0, "{e}");

    let (c, r, _) = rulec(&["test", &out]);
    assert_eq!(c, 0, "素の生成物が通らない:\n{r}");
    assert!(r.contains("ok    yupack_fee"), "{r}");
    assert!(r.contains("丸めヘルパ"), "丸めの単体ベクタも回す: {r}");

    // 生成物を手で壊したら赤になる。
    if which("python3").is_some() {
        let p = dir.join("python").join("yupack_fee.py");
        let src = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, tweak(&src, "運賃 = 4350", "運賃 = 4351", 1)).unwrap();
        let (c, r, _) = rulec(&["test", &out]);
        assert_eq!(c, 1, "壊れた生成物を通した:\n{r}");
        assert!(r.contains("FAIL  yupack_fee (Python)"), "{r}");
        assert!(r.contains("行目"), "何行目で食い違ったかを言う: {r}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn which(cmd: &str) -> Option<()> {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
        .then_some(())
}
