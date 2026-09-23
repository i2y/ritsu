//! Replay of the past (§10.2–§10.4). Acceptance for M3.
//!
//! No real data is needed. Building synthetic fixtures from the vectors and matching them against
//! a version with a seeded defect exercises the whole range: the separation of measured and
//! filled-in series, firing-row clusters, the rounding-difference tag, and the git-tag sugar. All
//! that stays with real data is the "validity" of the triage, which by design remains with
//! whoever holds the records, as the second danger in §16 says.

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

/// Build synthetic fixtures from the generated canonical vectors. The vector JSON has a shape we
/// decided ourselves (`{"in":…,"out":…,"trace":…`), so slicing by position is fine.
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

/// Prepare a workspace and put the vectors and synthetic fixtures in it.
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

/// A rewrite matched by content. If it does not match, it does not pass silently.
fn tweak(src: &str, from: &str, to: &str, want: usize) -> String {
    let n = src.matches(from).count();
    assert_eq!(n, want, "書き換えが当たっていない: `{from}` が {n} 箇所");
    src.replace(from, to)
}

/// The expected file `gen` writes beside the vectors is in the fixtures format (§15.34):
/// `fixtures lint` accepts it without a problem, and `replay` over it agrees with the rule
/// on every record — every vector is a record of the reference evaluator's own answer.
#[test]
fn 期待値のファイルはそのまま記録として通る() {
    let dir = std::env::temp_dir().join(format!("rulec-m3-{}-expected", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.to_string_lossy().to_string();
    for (rule, alias) in [(RULE, "yupack_fee"), ("tests/corpus/期間区分.rule", "period"), ("tests/corpus/会員特典.rule", "member_perk")] {
        let (c, _, e) = rulec(&["gen", rule, "--out", &out]);
        assert_eq!(c, 0, "{e}");
        let exp = dir.join("vectors").join(format!("{alias}.expected.jsonl"));
        let exp = exp.to_str().unwrap();
        let (c, o, e) = rulec(&["fixtures", "lint", exp, rule, "--format", "json"]);
        assert_eq!(c, 0, "{alias}: 期待値のファイルが記録として通らない: {o}{e}");
        assert!(o.contains("\"problems\":[]"), "{alias}: {o}");
        let (c, o, e) = rulec(&["replay", rule, "--fixtures", exp]);
        assert_eq!(c, 0, "{alias}: {o}{e}");
        assert!(o.contains("(100.000%)"), "{alias}: 期待値の再生が全件一致しない:\n{o}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A `trace` that names a table or a row the rule does not have is a record of some other
/// version, and it is reported by kind rather than read.
#[test]
fn 実在しない行を指す記録は種類つきで報告される() {
    let dir = setup("badtrace");
    let fx = std::fs::read_to_string(dir.join("fx.jsonl")).unwrap();
    let mut lines: Vec<String> = fx.lines().map(|l| l.to_string()).collect();
    let l0 = lines[0].strip_suffix('}').unwrap().to_string();
    lines[0] = format!("{l0},\"trace\":[{{\"table\":\"運賃表\",\"row\":99}}]}}");
    let l1 = lines[1].strip_suffix('}').unwrap().to_string();
    lines[1] = format!("{l1},\"trace\":[{{\"table\":\"存在しない表\",\"row\":1}}]}}");
    let l2 = lines[2].strip_suffix('}').unwrap().to_string();
    lines[2] = format!("{l2},\"trace\":[{{\"table\":\"運賃表\",\"row\":1}}]}}");
    std::fs::write(dir.join("fx.jsonl"), lines.join("\n") + "\n").unwrap();
    let fx = dir.join("fx.jsonl");
    let (_, out, _) = rulec(&["fixtures", "lint", fx.to_str().unwrap(), RULE, "--format", "json"]);
    assert!(out.contains("\"kind\":\"bad_trace\""), "{out}");
    assert!(out.contains("行99 はありません"), "{out}");
    assert!(out.contains("存在しない表 はこの規則にありません"), "{out}");
    // The third one is a real row, so it is read, not reported: only two problems.
    assert!(out.contains("\"count\":1"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A record that carries the rows that matched is compared row by row as well (§15.35). A
/// record whose value agrees but whose row differs is a moved row, apart from the mismatches
/// and clustered by the move; a mismatch's cluster names the move too.
#[test]
fn 記録の行が規則の行と違えば行の移動として出る() {
    let dir = std::env::temp_dir().join(format!("rulec-m3-{}-moved", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.to_string_lossy().to_string();
    let (c, _, e) = rulec(&["gen", RULE, "--out", &out]);
    assert_eq!(c, 0, "{e}");
    let exp = std::fs::read_to_string(dir.join("vectors").join("yupack_fee.expected.jsonl")).unwrap();
    // The first three records claim size row 7 (the last row, which none of the boundary
    // cases at the top of the file reaches); the fourth also has a different amount.
    // Everything else is left as the rule wrote it.
    let claim7 = |l: &str| -> String {
        let i = l.find("{\"table\":\"サイズ判定\",\"row\":").expect("サイズ判定 の行が無い");
        let j = i + l[i..].find('}').unwrap();
        assert!(!l[i..j].ends_with(":7"), "{l}");
        format!("{}{{\"table\":\"サイズ判定\",\"row\":7{}", &l[..i], &l[j..])
    };
    let mut lines: Vec<String> = exp.lines().map(|l| l.to_string()).collect();
    for l in lines.iter_mut().take(3) {
        *l = claim7(l);
    }
    let l3 = claim7(&lines[3]);
    let i = l3.find("\"運賃\":").unwrap() + "\"運賃\":".len();
    let j = i + l3[i..].find('}').unwrap();
    let v: i64 = l3[i..j].parse().unwrap();
    lines[3] = format!("{}{}{}", &l3[..i], v + 10, &l3[j..]);
    let fx = dir.join("fx.jsonl");
    std::fs::write(&fx, lines.join("\n") + "\n").unwrap();

    let (c, o, e) = rulec(&["replay", RULE, "--fixtures", fx.to_str().unwrap()]);
    // Exit 1: there is one real mismatch. The moved rows alone would not turn it on.
    assert_eq!(c, 1, "{o}{e}");
    assert!(o.contains("値は同じで、当てはまった行が記録と違う記録 3 件"), "{o}");
    assert!(o.contains("表 サイズ判定 行7→行1"), "移動が 記録→規則 の向きで出る:\n{o}");
    assert!(o.contains("影響 1 件"), "値の食い違いは一件だけ:\n{o}");
    let (_, j, _) = rulec(&["replay", RULE, "--fixtures", fx.to_str().unwrap(), "--format", "json"]);
    assert!(j.contains("\"moved\":[{\"rows\":[{\"table\":\"サイズ判定\",\"from\":7,\"to\":1}"), "{j}");
    assert!(j.contains(&format!("\"matched\":{}", lines.len() - 1)), "行の移動は一致に数える:\n{j}");
    let _ = std::fs::remove_dir_all(&dir);
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

/// §10.2: only type and range validation is taken on. Broken records are **reported, not
/// discarded**. Dropping them silently makes the match rate look higher by however much the
/// denominator shrank.
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
    // Records with a missing field (excluded entirely when there is no default value).
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
        // A decimal is refused at the field, naming it: §10.2 wants an integer in the
        // canonical unit, and the record says which field broke that.
        "`observed.運賃`: 決まった単位の整数 を期待しましたが 小数 でした",
        "規則が知らないフィールド",
        "列挙 都道府県 の値ではありません",
        // The range is written the way the rule writes it, and so is what the integer means.
        "900（900cm）は宣言範囲 1cm..170cm の外",
        "`observed.運賃` がありません",
    ] {
        assert!(out.contains(want), "`{want}` を言っていない:\n{out}");
    }
    assert!(out.contains("フィールドが欠けていたので外した記録: 5 件"), "{out}");
    // A witness (which line, which record) is required.
    assert!(out.contains("order:b3"), "証人を出す: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.3: the headline match rate comes from the measured series alone. Filling only affects the
/// secondary tally. And **the report itself records the defaults used and the fill count per
/// field**.
#[test]
fn 補った記録は分けて数え_使った既定値を書き残す() {
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

    // Without a default value, a record with a missing field is excluded entirely.
    let (_, plain, _) = rulec(&["replay", RULE, "--fixtures", fx]);
    assert!(plain.contains(&format!("照合 {base} 件")), "そのままの記録だけを数える: {plain}");
    assert!(plain.contains("フィールドが欠けていて既定値も無い記録を 4 件外しました"), "{plain}");

    // Filling through the manifest counts them separately as the filled-in series, and the default
    // value is recorded.
    let m = dir.join("m.json");
    std::fs::write(&m, r#"{"rulec":"replay/1","rule":"ゆうパック運賃","fills":{"重量":1000}}"#).unwrap();
    let (_, filled, _) =
        rulec(&["replay", RULE, "--fixtures", fx, "--manifest", m.to_str().unwrap()]);
    assert!(filled.contains(&format!("照合 {base} 件")), "補完を見出しに混ぜている: {filled}");
    assert!(filled.contains("補った記録 4 件（重量 4 件）"), "フィールドごとの件数を刻む: {filled}");
    assert!(filled.contains("使った既定値: 重量 = 1000"), "既定値を刻む: {filled}");

    // --fill is a temporary override for sensitivity analysis. It applies after the manifest.
    let (_, over, _) = rulec(&["replay", RULE, "--fixtures", fx, "--manifest", m.to_str().unwrap(), "--fill", "重量=2000"]);
    assert!(over.contains("使った既定値: 重量 = 2000"), "上書きが効いていない: {over}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.4: in a version where only a value changed, the firing rows stay the same and only the
/// amount moves.
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

/// §10.4: a version where a boundary moved shows up as a transition of firing rows.
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
    // The key is only the pair of firing rows (§10.4). Even when the old amounts differ, the same
    // transition is the same cluster.
    let clusters = out.lines().filter(|l| l.starts_with("  表 ")).count();
    let rows = out.lines().filter(|l| l.contains("行1→行2")).count();
    assert!(rows >= 5 && clusters == rows, "遷移ごとに一クラスタのはず: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.4: a cluster made only of deviations smaller than the output grid gets tagged as a
/// suspected rounding difference.
#[test]
fn 刻み未満のずれは丸め方の違いとして括られる() {
    let dir = setup("round");
    // Shift the recorded side by just 3 yen. The output grid is 10円, so this has the shape of a
    // difference in rounding convention.
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
    assert!(r.contains("丸め方の違いの疑い（出力の刻み 10円 未満"), "{r}");
    assert!(r.contains("差 +3 一様"), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §1.4: `送料@v3` is sugar for looking up the git tag `rules/送料/v3`.
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

    // §15.42: a path at any revision, which is what a pull request compares against, and the
    // name with a revision that is not a tag.
    let (c, out, e) = rulec_in(&repo, &["diff", "rules/ゆうパック運賃.rule@HEAD~1", rel, "--fixtures", "fx.jsonl"]);
    assert_eq!(c, 1, "{out}{e}");
    assert!(out.contains("影響 7 件"), "{out}");
    let (c, out, e) = rulec_in(&repo, &["diff", "ゆうパック運賃@HEAD~1", "ゆうパック運賃@HEAD", "--fixtures", "fx.jsonl"]);
    assert_eq!(c, 1, "{out}{e}");
    assert!(out.contains("影響 7 件"), "{out}");
    // Neither a tag nor a revision: the message names both attempts.
    let (c, _, e) = rulec_in(&repo, &["diff", "ゆうパック運賃@v9", rel, "--fixtures", "fx.jsonl"]);
    assert_eq!(c, 2, "{e}");
    assert!(e.contains("rules/ゆうパック運賃/v9") && e.contains("リビジョン `v9`"), "{e}");
    let (c, _, e) = rulec_in(&repo, &["diff", "rules/なし.rule@HEAD", rel, "--fixtures", "fx.jsonl"]);
    assert_eq!(c, 2, "{e}");
    assert!(e.contains("`HEAD` に `rules/なし.rule` が無い"), "{e}");

    // Trying to compare two different rules is stopped.
    std::fs::write(repo.join("other.rule"), std::fs::read_to_string(root().join("tests/corpus/送料.rule")).unwrap()).unwrap();
    let (c, _, e) = rulec_in(&repo, &["diff", "ゆうパック運賃@v1", "other.rule", "--fixtures", "fx.jsonl"]);
    assert_eq!(c, 2, "別の規則を通している");
    assert!(e.contains("別の規則"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §12: the form to paste into a PR. The tool takes care of the formatting; posting is left to a
/// single line in CI.
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
    assert!(out.contains("| 照合（そのままの記録） |"), "{out}");
    assert!(out.contains("#### 不一致の内訳"), "{out}");
    // The table must not break at cell separators.
    for l in out.lines().filter(|l| l.starts_with("| 表 ")) {
        assert_eq!(l.matches(" | ").count(), 3, "列がずれている: {l}");
    }

    // §15.42: `--terse` leaves the witness column out altogether, so no value of a record is
    // in what gets pasted into a pull request; the counts and amounts stay.
    let fx = dir.join("fx.jsonl");
    let (c, terse, _) = rulec(&["diff", RULE, v2.to_str().unwrap(), "--fixtures", fx.to_str().unwrap(), "--format", "markdown", "--terse"]);
    assert_eq!(c, 1);
    assert!(terse.contains("#### 不一致の内訳") && terse.contains("影響 7 件"), "{terse}");
    assert!(!terse.contains("入力例") && !terse.contains("あて先="), "{terse}");
    for l in terse.lines().filter(|l| l.starts_with("| 表 ")) {
        assert_eq!(l.matches(" | ").count(), 2, "列がずれている: {l}");
    }
    let (c, plain, _) = rulec(&["diff", RULE, v2.to_str().unwrap(), "--fixtures", fx.to_str().unwrap(), "--terse"]);
    assert_eq!(c, 1);
    assert!(plain.contains("影響 7 件") && !plain.contains("例:"), "{plain}");
    let (c, _, e) = rulec(&["diff", RULE, v2.to_str().unwrap(), "--fixtures", fx.to_str().unwrap(), "--format", "json", "--terse"]);
    assert_eq!(c, 2, "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §9.3-3, §12: the generated code can also be run in the user's CI.
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

    // Breaking the generated code by hand turns it red.
    if which("python3").is_some() {
        let p = dir.join("python").join("yupack_fee.py");
        let src = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, tweak(&src, "fee = 4350", "fee = 4351", 1)).unwrap();
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


/// A run that compares nothing is not a pass (§15.144). Records the rule can no longer read —
/// here every observed amount is outside the declared range — are all excluded, and "no
/// mismatches" over none of them used to end the run with exit 0.
#[test]
fn 一件も照合できなければ1で終わる() {
    let dir = setup("none");
    let p = dir.join("unreadable.jsonl");
    let rec = r#"{"tag":"order:x","in":{"あて先":"東京都","三辺合計":900,"重量":1000},"observed":{"運賃":820}}"#;
    std::fs::write(&p, format!("{rec}\n{rec}\n")).unwrap();
    let (c, out, _) = rulec(&["replay", RULE, "--fixtures", p.to_str().unwrap(), "--lang", "en"]);
    assert_eq!(c, 1, "照合 0 件で通ってしまった:\n{out}");
    assert!(out.contains("Not one record was compared") && !out.contains("No mismatches"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
