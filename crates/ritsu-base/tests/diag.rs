//! Diagnostics (DESIGN 4.2): the text in both languages, the place written for a person and the
//! path from the root for the JSON, the JSON's keys and their order, and a language's own part.

use ritsu_base::diag::{self, Diag, Extra, Severity};
use ritsu_base::json::Json;
use ritsu_base::text::{self, Lang, Text};
use ritsu_base::tr;

const SRC: &str = "requirements 例 v1\n  from @民法 第142条   \n";

fn sample() -> Diag {
    Diag::error("E302", "tests/x/例.req", 2, 3, tr!("民法 第142条 が変わりました", "民法 第142条 changed"))
        .rel("crates/yuen/tests/x/例.req")
        .source(SRC)
        .note(tr!("いまは sha256:54a319e4148c24c7 です", "it is sha256:54a319e4148c24c7 now"))
        .fix_line("  from @民法 第142条 sha256:54a319e4148c24c7  ")
}

#[test]
fn the_text_in_both_languages() {
    let d = sample();
    assert_eq!(
        d.render(Lang::En),
        "error[E302]: tests/x/例.req:2:3: 民法 第142条 changed\n     2 |   from @民法 第142条\n  = it is sha256:54a319e4148c24c7 now\n  = The line, fixed: from @民法 第142条 sha256:54a319e4148c24c7\n"
    );
    assert_eq!(
        d.render(Lang::Ja),
        "エラー[E302]: tests/x/例.req:2:3: 民法 第142条 が変わりました\n     2 |   from @民法 第142条\n  = いまは sha256:54a319e4148c24c7 です\n  = 直した行: from @民法 第142条 sha256:54a319e4148c24c7\n"
    );
}

#[test]
fn the_json_keys_in_their_order_with_the_path_from_the_root() {
    let j = sample().to_json(Lang::En);
    let keys: Vec<&str> = j.as_obj().unwrap().iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["code", "severity", "file", "line", "col", "message", "notes", "fix"]);
    assert_eq!(
        j.compact(),
        r#"{"code":"E302","severity":"error","file":"crates/yuen/tests/x/例.req","line":2,"col":3,"message":"民法 第142条 changed","notes":["it is sha256:54a319e4148c24c7 now"],"fix":"  from @民法 第142条 sha256:54a319e4148c24c7  "}"#
    );
    // A whole file has no line or column: null, not 0.
    let w: Diag = Diag::whole("W101", "a.req", tr!("ファイル", "a file"));
    let j = w.to_json(Lang::Ja);
    assert_eq!(j.get("line"), Some(&Json::Null));
    assert_eq!(j.get("severity").and_then(Json::as_str), Some("warning"));
    assert_eq!(w.render(Lang::Ja), "警告[W101]: a.req: ファイル\n");
    // A tool puts the root beside its diagnostics.
    let report = Json::obj([("root", Json::str("../..")), ("diagnostics", Json::arr([sample().to_json(Lang::En)]))]);
    assert!(report.compact().starts_with(r#"{"root":"../..","diagnostics":[{"code":"E302""#));
}

#[test]
fn places_severities_and_counts() {
    let at_line: Diag = Diag::at("N101", "m.ctx", 4, 0, Text::same("x"));
    assert_eq!(at_line.place(), "m.ctx:4");
    assert_eq!(at_line.severity, Severity::Note);
    assert_eq!(at_line.render(Lang::Ja), "備考[N101]: m.ctx:4: x\n");
    assert_eq!(Severity::of("W203"), Severity::Warning);
    assert_eq!(Severity::of("E001"), Severity::Error);
    let ds: Vec<Diag> = vec![sample(), Diag::warning("W1", "a", 1, 1, Text::same("w")), at_line];
    let c = diag::count(&ds);
    assert_eq!((c.errors, c.warnings, c.notes), (1, 1, 1));
    assert!(diag::has_errors(&ds));
    assert!(!diag::has_errors(&ds[1..]));
}

#[test]
fn a_fix_a_note_already_said_and_a_command_to_run() {
    let d: Diag = Diag::error("E111", "t.cal", 3, 1, Text::same("not pinned")).note(Text::same("pin it: 第142条 sha256:fc8c35a0769d3b35")).fix_in_notes("  第142条 sha256:fc8c35a0769d3b35");
    assert!(!d.render(Lang::En).contains("The line, fixed"), "the note says it");
    assert_eq!(d.to_json(Lang::En).get("fix").and_then(Json::as_str), Some("  第142条 sha256:fc8c35a0769d3b35"));
    let d: Diag = Diag::error("E302", "a.req", 1, 1, Text::same("changed")).fix_command(tr!("yuen review a.req --at a.req:1 --by <役割>", "yuen review a.req --at a.req:1 --by <role>"));
    assert!(d.render(Lang::En).ends_with("  = Once a person has looked: yuen review a.req --at a.req:1 --by <role>\n"));
    assert!(d.render(Lang::Ja).ends_with("  = 確かめたら: yuen review a.req --at a.req:1 --by <役割>\n"));
}

/// A language's own part, as yuen's chain goes before the fix and koyomi's computation after.
#[derive(Clone, Debug, Default)]
struct Steps {
    chain: Vec<String>,
    steps: Vec<String>,
}

impl Extra for Steps {
    fn before_fix(&self, lang: Lang, out: &mut String) {
        if !self.chain.is_empty() {
            out.push_str(if lang == Lang::Ja { "  つながり:\n" } else { "  the chain:\n" });
            for c in &self.chain {
                out.push_str(&format!("      {c}\n"));
            }
        }
    }
    fn after_fix(&self, lang: Lang, out: &mut String) {
        if !self.steps.is_empty() {
            out.push_str(if lang == Lang::Ja { "  計算:\n" } else { "  the steps:\n" });
            for s in &self.steps {
                out.push_str(&format!("      {s}\n"));
            }
        }
    }
    fn json(&self, _lang: Lang) -> Vec<(String, Json)> {
        vec![("chain".into(), Json::arr(self.chain.iter().map(Json::str))), ("steps".into(), Json::arr(self.steps.iter().map(Json::str)))]
    }
    fn say(&self, t: &Text, lang: Lang) -> String {
        text::spaced(t, lang)
    }
}

#[test]
fn a_languages_own_part_goes_where_it_says() {
    let d: Diag<Steps> = Diag::error("E201", "t.cal", 1, 1, tr!("dateは無い日に当たります", "date can land on a missing day"))
        .fix_line("  + 1 month else end_of_month")
        .with(Steps { chain: vec!["a → b".into()], steps: vec!["2026-01-31  + 1 month".into()] });
    assert_eq!(
        d.render(Lang::En),
        "error[E201]: t.cal:1:1: Date can land on a missing day\n  the chain:\n      a → b\n  = The line, fixed: + 1 month else end_of_month\n  the steps:\n      2026-01-31  + 1 month\n"
    );
    assert!(d.render(Lang::Ja).starts_with("エラー[E201]: t.cal:1:1: date は無い日に当たります\n"), "{}", d.render(Lang::Ja));
    let keys: Vec<String> = d.to_json(Lang::En).as_obj().unwrap().iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(keys, ["code", "severity", "file", "line", "col", "message", "notes", "chain", "steps", "fix"]);
}
