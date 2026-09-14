//! rulec — checks decision tables of business rules and generates code for five languages.
//! The design is in DESIGN.md.

/// A user-facing sentence in both languages: `tr!("日本語", "English")`.
///
/// Expands to a `String` in the current output language (`i18n`). Both texts are
/// `format!` strings and receive the same arguments, so `tr!("{} 件", "{} items", n)`
/// and inline captures such as `tr!("{name} が…", "{name} is…")` both work.
/// Keep the two texts adjacent: a message that exists in only one language is
/// a bug, and the compiler checks the placeholders of both.
#[macro_export]
macro_rules! tr {
    ($ja:literal, $en:literal) => {
        if $crate::i18n::ja() { ::std::format!($ja) } else { ::std::format!($en) }
    };
    ($ja:literal, $en:literal, $($arg:tt)+) => {
        if $crate::i18n::ja() { ::std::format!($ja, $($arg)+) } else { ::std::format!($en, $($arg)+) }
    };
}

pub mod ast;
pub mod diag;
pub mod doc;
pub mod eval;
pub mod fixtures;
pub mod fmt;
pub mod i18n;
pub mod codegen;
pub mod codes;
pub mod coverage;
pub mod json;
pub mod kw;
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

/// Checks one `.rule` and returns its diagnostics. There are three stages, and when a
/// stage reports an error the next one does not run (type failures stacked on top of
/// syntax failures are unreadable).
pub struct Report {
    pub diags: Vec<Diag>,
    /// Total number of nodes the check visited (measured to set the §15-2 budget).
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

/// Per-table check results (shadowing pairs and dead rows) that the §9.2 coverage decision
/// needs. Returned in table order.
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

/// Splits the E104 wording in two. The branch is not a guess: witnesses are actually
/// evaluated and the choice depends on whether a fraction appears. An output that is only
/// looked up from a table never produces a fraction, so for it the second job of the
/// rounding declaration (checking that literals sit on the grid) is given as the reason.
fn enrich_e104(diags: &mut [diag::Diag], f: &ast::RuleFile, t: &types::Checked) {
    use num::{Rat, RoundMode};
    let Some(raw) = eval::unrounded_output(f, t) else { return };
    for d in diags.iter_mut().filter(|d| d.code == "E104") {
        // Only form A (a fraction can occur) says "an unrounded value reaches the output".
        // Giving form B the same first line would claim a leak where nothing leaks.
        if !raw.is_int() {
            d.title = tr!("丸めていない値が出力に到達します", "An unrounded value reaches the output");
        }
        if raw.is_int() {
            d.notes.push(tr!(
                "入力をいくつか試しましたが、この出力に端数は生まれませんでした（表から引いた額がそのまま出るか、式の中で既に丸めているためです）。",
                "Several witnesses were tried and none produced a fraction in this output (either the amount looked up from the table is emitted as is, or the expression already rounds it)."
            ));
            d.notes.push(tr!(
                "丸めの宣言はここでは第二の働きをします。出力セルのリテラルがその刻みに載っているかを検査するのに使われ、1451円 のような桁の打ち間違いが E106 で止まります。",
                "Here the rounding declaration does its second job: it is used to check that the literals in the output cells sit on that grid, so a mistyped digit such as 1451円 is stopped by E106."
            ));
            d.notes.push(tr!(
                "ヒント: 出力の宣言に丸めを書いてください。例: {} {}(10円)",
                "Hint: add rounding to the output declaration, e.g. {} {}(10円)",
                crate::kw::ROUND, crate::kw::UP
            ));
        } else {
            d.notes.push(tr!(
                "端数がどう決まるかを宣言しないと、生成コードが黙って決めます。",
                "Unless you declare how the fraction is settled, the generated code decides silently."
            ));
            let one = Rat::int(1);
            let ten = Rat::int(10);
            let down = raw.round_to(RoundMode::Down, one);
            let half = raw.round_to(RoundMode::Half, one);
            let up10 = raw.round_to(RoundMode::Up, ten);
            let spread = up10.sub(down);
            d.notes.push(tr!(
                "例: 計算値が {raw} 円になる入力があります。{}(1円) なら {down}円、{}(1円) なら {half}円、{}(10円) なら {up10}円 と、丸め方で最大 {spread}円 動きます。",
                "Example: some input computes to {raw} yen. {}(1円) gives {down} yen, {}(1円) gives {half} yen and {}(10円) gives {up10} yen, so the rounding mode moves the result by up to {spread} yen.",
                crate::kw::DOWN, crate::kw::HALF_UP, crate::kw::UP
            ));
            d.notes.push(tr!(
                "ヒント: 出力の宣言に丸めを書いてください。例: {} {}(10円)",
                "Hint: add rounding to the output declaration, e.g. {} {}(10円)",
                crate::kw::ROUND, crate::kw::UP
            ));
        }
    }
}

/// Gathers everything generation needs in one go. Nothing is generated if syntax or types
/// fail.
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
