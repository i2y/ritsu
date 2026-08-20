//! rulec — 業務ルールの決定表を検査し、Python と Go に生成する。設計は DESIGN.md。

pub mod ast;
pub mod diag;
pub mod eval;
pub mod fixtures;
pub mod fmt;
pub mod codegen;
pub mod coverage;
pub mod json;
pub mod lex;
pub mod num;
pub mod parse;
pub mod prelude;
pub mod region;
pub mod replay;
pub mod report;
pub mod runtest;
pub mod types;
pub mod vectors;
pub mod verify;

use diag::{Diag, Severity};

/// 一つの `.rule` を検査して診断を返す。段は三つで、前の段がエラーを出したら
/// 次に進まない（構文の失敗の上に型の失敗を積んでも読めない）。
pub struct Report {
    pub diags: Vec<Diag>,
    /// 検査で訪問したノード数の合計（§15-2 の予算を決めるための実測）。
    pub nodes: i64,
    pub quiet: Vec<Diag>,
    pub shadow: region::Shadow,
}

pub fn check_source(src: &str, path: &str) -> Vec<Diag> {
    report(src, path).diags
}

pub fn report(src: &str, path: &str) -> Report {
    report_with(src, path, region::DEFAULT_BUDGET)
}

pub fn report_with(src: &str, path: &str, budget: i64) -> Report {
    let parsed = parse::parse(src, path);
    let mut diags = parsed.diags.clone();
    let mut quiet = Vec::new();
    let mut shadow = region::Shadow::default();
    let mut nodes = 0i64;
    if !diags.is_empty() {
        return Report { diags, quiet, shadow, nodes };
    }
    let Some(f) = &parsed.file else { return Report { diags, quiet, shadow, nodes } };

    let t = types::check(f, path);
    diags.extend(t.diags.iter().cloned());
    enrich_e104(&mut diags, f, &t);
    if diags.iter().any(|d| d.severity == Severity::Error) {
        return Report { diags, quiet, shadow, nodes };
    }
    for it in &f.items {
        if let ast::Item::Table(tb) = it {
            let r = region::check_table(tb, &t, f, path, budget);
            diags.extend(r.diags);
            quiet.extend(r.quiet);
            shadow.structural += r.shadow.structural;
            shadow.equivalent += r.shadow.equivalent;
            shadow.confirm += r.shadow.confirm;
            nodes += r.nodes;
        }
    }
    diags.extend(eval::check_examples(f, &t, path));
    Report { diags, quiet, shadow, nodes }
}

/// §9.2 の被覆判定に要る、表ごとの検査結果（遮蔽対と死行）。表の並び順で返す。
pub fn table_checks(
    f: &ast::RuleFile,
    c: &types::Checked,
    path: &str,
) -> Vec<region::TableCheck> {
    f.items
        .iter()
        .filter_map(|it| match it {
            ast::Item::Table(tb) => {
                Some(region::check_table(tb, c, f, path, region::DEFAULT_BUDGET))
            }
            _ => None,
        })
        .collect()
}

pub fn has_error(ds: &[Diag]) -> bool {
    ds.iter().any(|d| d.severity == Severity::Error)
}

/// E104 の文面を二分する。分岐は当て推量ではなく、証人を実際に評価して
/// 端数が出るかで決める。表引きだけの出力では端数が出ないので、
/// 丸め宣言の第二の働き（リテラルが格子に載るかの検査）を理由に据える。
fn enrich_e104(diags: &mut [diag::Diag], f: &ast::RuleFile, t: &types::Checked) {
    use num::{Rat, RoundMode};
    let Some(raw) = eval::unrounded_output(f, t) else { return };
    for d in diags.iter_mut().filter(|d| d.code == "E104") {
        // A 形（端数が生じうる）だけが「丸めていない値が到達する」。
        // B 形で同じ一行目を出すと、何も漏れていないのに漏れたと言うことになる。
        if !raw.is_int() {
            d.title = "丸めていない値が出力に到達します".into();
        }
        if raw.is_int() {
            d.notes.push("証人をいくつか試しましたが、この出力に端数は生まれませんでした（表から引いた額がそのまま出るか、式の中で既に丸めているためです）。".into());
            d.notes.push("丸めの宣言はここでは第二の働きをします。出力セルのリテラルがその格子に載っているかを検査するのに使われ、1451円 のような桁の打ち間違いが E106 で止まります。".into());
            d.notes.push("ヒント: 出力の宣言に丸めを書いてください。例: 丸め 切り上げ(10円)".into());
        } else {
            d.notes.push("端数がどう決まるかを宣言しないと、生成コードが黙って決めます。".into());
            let one = Rat::int(1);
            let ten = Rat::int(10);
            let down = raw.round_to(RoundMode::Down, one);
            let half = raw.round_to(RoundMode::Half, one);
            let up10 = raw.round_to(RoundMode::Up, ten);
            let spread = up10.sub(down);
            d.notes.push(format!(
                "例: 計算値が {raw} 円になる入力があります。切り捨て(1円) なら {down}円、四捨五入(1円) なら {half}円、切り上げ(10円) なら {up10}円 と、丸め方で最大 {spread}円 動きます。"
            ));
            d.notes.push("ヒント: 出力の宣言に丸めを書いてください。例: 丸め 切り上げ(10円)".into());
        }
    }
}

/// 生成に必要なものを一度に揃える。構文か型で落ちたら生成しない。
pub fn prepare<'a>(src: &'a str, path: &str) -> Result<(ast::RuleFile, types::Checked), Vec<Diag>> {
    let parsed = parse::parse(src, path);
    if !parsed.diags.is_empty() {
        return Err(parsed.diags);
    }
    let Some(f) = parsed.file else { return Err(Vec::new()) };
    let t = types::check(&f, path);
    if t.diags.iter().any(|d| d.severity == Severity::Error) {
        return Err(t.diags);
    }
    Ok((f, t))
}
