//! rulec — checks decision tables of business rules and generates code for every target
//! [`backend::ALL`] names.
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
pub mod apply;
pub mod backend;
pub mod cel;
pub mod cert;
pub mod defset;
pub mod diag;
pub mod doc;
pub mod docx;
pub mod enums;
pub mod eval;
pub mod fixtures;
pub mod fmt;
pub mod fourier;
pub mod grid;
pub mod graph;
pub mod i18n;
pub mod extract;
pub mod import;
pub mod codegen;
pub mod codes;
pub mod coverage;
pub mod json;
pub mod jsonschema;
pub mod kw;
pub mod lex;
pub mod machine;
pub mod num;
pub mod ooxml;
pub mod parse;
pub mod prelude;
pub mod proto;
pub mod region;
pub mod relation;
pub mod replay;
pub mod projection;
pub mod report;
pub mod runtest;
pub mod sha256;
pub mod sources;
pub mod types;
pub mod vfs;
pub mod vdiff;
pub mod vectors;
pub mod verify;
pub mod xlsx;
/// The library as a web page. Only built for the target the site loads (§15.48).
#[cfg(target_arch = "wasm32")]
pub mod wasm;

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
    let Some(mut f) = parsed.file else { return Report { diags, quiet, shadow, nodes } };
    // A rule applied by this one is read and expanded into it first (§15.69). A callee that
    // cannot be expanded leaves holes everything below would trip over, so those errors
    // return here; a pin that does not match (E040) is reported and the check goes on, so
    // that what the changed callee does to this rule can be seen.
    diags.extend(apply::expand(&mut f, path));
    if diags.iter().any(|d| d.severity == Severity::Error && d.code != "E040") {
        return Report { diags, quiet, shadow, nodes };
    }
    let f = &f;

    let t = types::check(f, path);
    diags.extend(t.diags.iter().cloned());
    diags.extend(apply::check(f, &t, path));
    // The stages that read other files: the enums declared outside it (§15.59, §15.60), and
    // the copies of the sources it cites (§15.68).
    diags.extend(enums::check(f, &t, path));
    diags.extend(projection::check(f, &t, path));
    diags.extend(sources::check(f, path));
    enrich_e104(&mut diags, f, &t);
    if diags.iter().any(|d| d.severity == Severity::Error) {
        return Report { diags, quiet, shadow, nodes };
    }
    // The unit is the definition set: one table, or every table that defines one output
    // (§15.66).
    for set in &t.sets {
        let r = region::check_set(set, &t, f, path, budget);
        diags.extend(r.diags);
        quiet.extend(r.quiet);
        shadow.structural += r.shadow.structural;
        shadow.equivalent += r.shadow.equivalent;
        shadow.confirm += r.shadow.confirm;
        nodes += r.nodes;
    }
    diags.extend(eval::check_examples(f, &t, path));
    // The machine the rule is one step of (§15.148). Its claims are about sequences of calls,
    // and a step with a hole or two answers is not a step yet: they wait for the tables.
    let tables_hold = !diags.iter().any(|d| d.severity == Severity::Error && d.code.starts_with("E1") && d.code != "E107");
    if f.machine.is_some() {
        diags.extend(machine::check_scenarios(f, &t, path));
        if tables_hold {
            diags.extend(machine::check(f, &t, path, budget.max(0) as usize));
        }
    }
    Report { diags, quiet, shadow, nodes }
}

/// Per-set check results (shadowing pairs and dead rows) that the §9.2 coverage decision
/// needs. Returned in the order of `c.sets`.
pub fn table_checks(
    f: &ast::RuleFile,
    c: &types::Checked,
    path: &str,
) -> Vec<region::TableCheck> {
    c.sets.iter().map(|s| region::check_set(s, c, f, path, region::DEFAULT_BUDGET)).collect()
}

pub fn has_error(ds: &[Diag]) -> bool {
    ds.iter().any(|d| d.severity == Severity::Error)
}

/// Every finding of one file, in the frame §11 fixes, with a blank line between them.
///
/// `main` walks the list itself (it also counts, and `--format json` and `--terse` are
/// other renderings of the same list), and prints this for the default one; the wasm
/// playground (§15.48) returns it. Two implementations would be two sets of answers to
/// keep true.
pub fn findings_text(diags: &[Diag], lines: &[String]) -> String {
    let mut out = String::new();
    for d in diags {
        out.push_str(&diag::render(d, lines));
        out.push('\n');
    }
    out
}

/// What `check` prints after the findings: the count of findings the base revision already
/// had (`--diff-base`), the count of shadowing pairs (§4), and the `ok` line when nothing
/// is an error. `--format json` prints none of it; `--terse` prints all of it.
pub fn check_tail(shadow: &region::Shadow, diags: &[Diag], path: &str, suppressed: usize) -> String {
    let mut out = String::new();
    if suppressed > 0 {
        out.push_str(&tr!(
            "note {path}: 基準リビジョンに既にあった指摘 {suppressed} 件は伏せました（--diff-base）\n",
            "note {path}: suppressed {suppressed} findings already present at the base revision (--diff-base)\n"
        ));
    }
    // §4: only the pairs that need review are listed; the rest is a single count line.
    if shadow.total() > 0 {
        out.push_str(&tr!(
            "note {path}: 隠れ {} 対（階段 {}、同じ答え {}、要確認 {}）\n",
            "note {path}: {} shadow pairs ({} structural, {} equivalent, {} needs review)\n",
            shadow.total(),
            shadow.structural,
            shadow.equivalent,
            shadow.confirm
        ));
    }
    if !has_error(diags) {
        out.push_str(&format!("ok {path}\n"));
    }
    out
}

/// Splits the E104 wording in two. The branch is not a guess: witnesses are actually
/// evaluated and the choice depends on whether a fraction appears. An output that is only
/// looked up from a table never produces a fraction, so for it the second job of the
/// rounding declaration (checking that literals sit on the grid) is given as the reason.
fn enrich_e104(diags: &mut [diag::Diag], f: &ast::RuleFile, t: &types::Checked) {
    use num::{Rat, RoundMode};
    let Some(raw) = eval::unrounded_output(f, t) else { return };
    // The example is about this rule's own output, so it is written in that output's unit.
    // It used to be yen whatever the rule counted, which told the writer of a rule in dollars
    // that "some input computes to 35.5 yen" and to try `round up(10円)` — a message about a
    // currency the rule does not have (§15.111). A rate is held as a fraction and written in
    // percent, so it is put in percent before anything is said about it: 12% used to come out
    // as "0.12%", rounded as if it were 0.12%.
    let unit = unit_word(f, t);
    let u = unit.as_str();
    let first = f.outputs.first().map(|o| o.name.text.as_str()).unwrap_or("");
    let rate = matches!(t.syms.get(first).map(|s| &s.ty), Some(types::Ty::Rate));
    let scale = t.wire_scale(first);
    // A fraction of the grid the output is stored on: a whole yen, one step of a rate.
    let whole = raw.mul(Rat::int(scale)).is_int();
    let raw = if rate { raw.mul(Rat::int(100)) } else { raw };
    // The grid a hint names: a rate's own step, ten of anything else.
    let grid = if rate { format!("{}%", Rat::new(100, scale)) } else { format!("10{u}") };
    let typo = if rate { "1.451%".to_string() } else { format!("1451{u}") };
    for d in diags.iter_mut().filter(|d| d.code == "E104") {
        // Only form A (a fraction can occur) says "an unrounded value reaches the output".
        // Giving form B the same first line would claim a leak where nothing leaks.
        if !whole {
            d.title = tr!("丸めていない値が出力に到達します", "An unrounded value reaches the output");
        }
        if whole {
            d.notes.push(tr!(
                "入力をいくつか試しましたが、この出力に端数は生まれませんでした（表から引いた額がそのまま出るか、式の中で既に丸めているためです）。",
                "Several witnesses were tried and none produced a fraction in this output (either the amount looked up from the table is emitted as is, or the expression already rounds it)."
            ));
            d.notes.push(tr!(
                "丸めの宣言はここでは第二の働きをします。出力セルのリテラルがその刻みに載っているかを検査するのに使われ、{typo} のような桁の打ち間違いが E106 で止まります。",
                "Here the rounding declaration does its second job: it is used to check that the literals in the output cells sit on that grid, so a mistyped digit such as {typo} is stopped by E106."
            ));
            d.notes.push(tr!(
                "ヒント: 出力の宣言に丸めを書いてください。例: {} {}({grid})",
                "Hint: add rounding to the output declaration, e.g. {} {}({grid})",
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
                "例: 計算値が {raw}{u} になる入力があります。{}(1{u}) なら {down}{u}、{}(1{u}) なら {half}{u}、{}(10{u}) なら {up10}{u} と、丸め方で最大 {spread}{u} 動きます。",
                "Example: some input computes to {raw}{u}. {}(1{u}) gives {down}{u}, {}(1{u}) gives {half}{u} and {}(10{u}) gives {up10}{u}, so the rounding mode moves the result by up to {spread}{u}.",
                crate::kw::DOWN, crate::kw::HALF_UP, crate::kw::UP
            ));
            d.notes.push(tr!(
                "ヒント: 出力の宣言に丸めを書いてください。例: {} {}({grid})",
                "Hint: add rounding to the output declaration, e.g. {} {}({grid})",
                crate::kw::ROUND, crate::kw::UP
            ));
        }
    }
}

/// The unit the first numeric output counts in, as a literal writes it: `円`, `USD`, `g`,
/// `%`. A `number` counts nothing, so it is empty and the messages read as bare numbers.
fn unit_word(f: &ast::RuleFile, t: &types::Checked) -> String {
    let first = f.outputs.first().map(|o| o.name.text.as_str()).unwrap_or("");
    match t.syms.get(first).map(|s| &s.ty) {
        Some(types::Ty::Money { cur, .. }) => cur.clone(),
        Some(types::Ty::Qty { unit, .. }) => unit.clone(),
        Some(types::Ty::Rate) => "%".to_string(),
        _ => String::new(),
    }
}

/// Gathers everything generation needs in one go. Nothing is generated if syntax or types
/// fail.
pub fn prepare<'a>(src: &'a str, path: &str) -> Result<(ast::RuleFile, types::Checked), Vec<Diag>> {
    prepare_with(src, path, false)
}

/// [`prepare`], with an unpinned or changed callee (E040) let through: what `rulec diff`
/// needs, since the difference a changed callee makes is what E040 asks to be looked at.
pub fn prepare_lenient<'a>(src: &'a str, path: &str) -> Result<(ast::RuleFile, types::Checked), Vec<Diag>> {
    prepare_with(src, path, true)
}

fn prepare_with(src: &str, path: &str, tolerate_pin: bool) -> Result<(ast::RuleFile, types::Checked), Vec<Diag>> {
    let parsed = parse::parse(src, path);
    if !parsed.diags.is_empty() {
        return Err(parsed.diags);
    }
    let Some(mut f) = parsed.file else { return Err(Vec::new()) };
    let ds = apply::expand(&mut f, path);
    if ds.iter().any(|d| d.severity == Severity::Error && !(tolerate_pin && d.code == "E040")) {
        return Err(ds);
    }
    let t = types::check(&f, path);
    if t.diags.iter().any(|d| d.severity == Severity::Error) {
        return Err(t.diags);
    }
    let ds = apply::check(&f, &t, path);
    if ds.iter().any(|d| d.severity == Severity::Error) {
        return Err(ds);
    }
    Ok((f, t))
}
