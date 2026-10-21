//! What yuen writes into a `.req` is read back by its own check (DESIGN 21): the pin lines `yuen
//! source pin` adds under an OpenSpec spec and under a law, and the line a diagnostic gives as the
//! fix of a requirement or an article cited and not pinned (E102). A name is written bare only
//! where the lexer reads it back as that name, and in quotes everywhere else. Each project is
//! pinned (by the command, or by writing the fix the check gives), `review --all` writes its
//! records, and the check after it passes.
//!
//! The names of the OpenSpec specs are the shapes of the reproduction: the ones `source pin` wrote
//! bare and its check then stopped at (E001) — a hyphen, a dot, an apostrophe, an ASCII digit
//! first — and the ones it always wrote right. English names come first, Japanese names of the
//! same shapes beside them. The articles of a law are the forms a `.req` cites: sections of the
//! eCFR (`"§1.7"`, `"1.6"`), and an article, an appended table and supplementary provisions of a
//! law of e-Gov (`第142条`, `別表第一`, `附則第3条`, `"附則（令和七年三月三一日法律第一三号）第3条"`).

mod common;

use common::{Ran, TempDir};
use std::path::{Path, PathBuf};

/// A name, and whether a `.req` writes it bare.
type Spelled = (&'static str, bool);

/// The sixteen names of the reproduction, then a word of the language: a pin line and a citation
/// take any word, so `reviewed` is written bare as well.
const ENGLISH: &[Spelled] = &[
    ("Re-quote", false),
    ("e-mail", false),
    ("kebab-case-name", false),
    ("Re.quote", false),
    ("O'Brien", false),
    ("2FA", false),
    ("Opt in", false),
    ("Follow-up call", false),
    ("A/B test", false),
    ("v2.0 export", false),
    ("Price (list)", false),
    ("dealer_kind", true),
    ("Re_quote", true),
    ("Café", true),
    ("_private", true),
    ("if", true),
    ("reviewed", true),
];

/// The same shapes in Japanese, then two that only Japanese text has: a `・` and a full-width
/// space. A name that starts with ASCII digits and goes on in Japanese (`2段階認証`) is a name to
/// the lexer, as `142条` is, so it is written bare where `2FA` is not.
const JAPANESE: &[Spelled] = &[
    ("再-見積", false),
    ("電子-メール", false),
    ("見積-再-送付", false),
    ("見積.再", false),
    ("ロックン'ロール", false),
    ("2段階認証", true),
    ("オプト イン", false),
    ("フォロー-アップ の電話", false),
    ("A/Bテスト", false),
    ("第2.0版 の書き出し", false),
    ("価格（定価）", false),
    ("販売店_種別", true),
    ("再_見積", true),
    ("再見積", true),
    ("_内部", true),
    ("もし", true),
    ("見積・再", false),
    ("見積\u{3000}再", false),
];

fn quoted(s: &str) -> String {
    yuen::names::quote(s)
}

fn spelled(name: &str, bare: bool) -> String {
    if bare { name.to_string() } else { quoted(name) }
}

/// What a project is written in: its words, the role that owns its requirement and looks, and the
/// language the commands speak, with what `source pin` says when it adds a pin line.
struct Words {
    lang: &'static str,
    added: &'static str,
    description: &'static str,
    role: &'static str,
    role_says: &'static str,
    text: &'static str,
    waiver: &'static str,
}

const EN: Words = Words { lang: "en", added: "added a pin line for the cited {}", description: "What source pin writes, read back", role: "sales", role_says: "Owns it", text: "It does what the source says", waiver: "A test of the pin lines" };
const JA: Words = Words { lang: "ja", added: "引いている {} の固定の行を足しました", description: "source pin が書く行を読み直す", role: "営業", role_says: "持ち主", text: "出典の言うとおりにする", waiver: "固定の行の試験" };

/// A `.req` with one source and one requirement that cites it, no pin line yet: both waivers are
/// there, so that once the link is pinned and looked at the check passes.
fn req(w: &Words, source: &str, declared: &str, cited: &str) -> String {
    let Words { description, role, role_says, text, waiver, .. } = w;
    format!(
        "requirements s v1\ndescription \"{description}\"\n\nrole {role} \"{role_says}\"\n\nsource {source} = {declared}\n\nrequirement r\n  text \"{text}\"\n  owner {role}\n  from @{source} {cited}\n  not satisfied \"{waiver}\"\n    approved 2026-10-21 by {role}\n  not verified \"{waiver}\"\n    approved 2026-10-21 by {role}\n"
    )
}

/// A project of one OpenSpec spec, whose one requirement is `name`, and a `.req` that cites it.
fn spec_project(t: &TempDir, w: &Words, name: &str) -> PathBuf {
    let d = t.path().join("p");
    std::fs::create_dir_all(d.join("openspec/specs/s")).unwrap();
    let spec = format!("# S Specification\n\n## Purpose\nOne requirement.\n\n## Requirements\n\n### Requirement: {name}\nThe service SHALL do it.\n\n#### Scenario: does it\n- **WHEN** asked\n- **THEN** it does it\n");
    std::fs::write(d.join("openspec/specs/s/spec.md"), spec).unwrap();
    std::fs::write(d.join("s.req"), req(w, "spec", "openspec \"openspec/specs/s/spec.md\"", &quoted(name))).unwrap();
    d
}

/// The command run on the project's `.req`, its directory the root, in the project's language.
fn on(d: &Path, w: &Words, args: &[&str]) -> Ran {
    let file = d.join("s.req");
    let mut a: Vec<&str> = args.to_vec();
    a.extend([file.to_str().unwrap(), "--root", d.to_str().unwrap(), "--lang", w.lang]);
    common::run(&a)
}

/// The lines under the `source` line: the pins.
fn pins(d: &Path) -> Vec<String> {
    let src = std::fs::read_to_string(d.join("s.req")).unwrap();
    src.lines().skip_while(|l| !l.starts_with("source ")).skip(1).take_while(|l| l.starts_with("  ")).map(str::to_string).collect()
}

/// Every pin written the way the check gives it as the fix of E102, under the `source` line: the
/// fix is the line itself, trimmed, and the pins of a source are indented by two.
fn write_the_fixes(d: &Path, w: &Words) -> Vec<String> {
    let r = on(d, w, &["check", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap_or_else(|e| panic!("{e}: {}{}", r.stdout, r.stderr));
    let fixes: Vec<String> = v["diagnostics"].as_array().unwrap().iter().filter(|x| x["code"] == "E102").map(|x| x["fix"].as_str().unwrap().to_string()).collect();
    let file = d.join("s.req");
    let src = std::fs::read_to_string(&file).unwrap();
    let mut out = String::new();
    for l in src.split_inclusive('\n') {
        out.push_str(l);
        if l.starts_with("source ") {
            for f in &fixes {
                out.push_str(&format!("  {f}\n"));
            }
        }
    }
    std::fs::write(&file, out).unwrap();
    fixes
}

/// The project looked at and checked: what `review --all` writes, and a check that passes.
fn passes(d: &Path, w: &Words, what: &str) {
    let r = on(d, w, &["check"]);
    let codes = common::codes(&r.stdout);
    assert!(codes.iter().all(|c| c == "E301" || c == "E305"), "{what}: the check reads every line, and asks only for the records\n{}{}", r.stdout, r.stderr);
    let r = on(d, w, &["review", "--all", "--by", w.role, "--date", "2026-10-21"]);
    assert_eq!(r.code, 0, "{what}: {}{}", r.stdout, r.stderr);
    let r = on(d, w, &["check"]);
    assert_eq!(r.code, 0, "{what}: {}{}", r.stdout, r.stderr);
}

fn spec_names(w: &Words, names: &[Spelled]) {
    for &(name, bare) in names {
        let t = TempDir::new("pin-round-trip");
        let d = spec_project(&t, w, name);
        let block = ritsu_base::openspec::read_spec(&std::fs::read(d.join("openspec/specs/s/spec.md")).unwrap()).unwrap().get(name).unwrap().block.clone();
        let line = format!("  {} sha256:{}", spelled(name, bare), ritsu_base::sha256::short(block.as_bytes()));
        let r = on(&d, w, &["source", "pin"]);
        assert_eq!(r.code, 0, "{name}: {}{}", r.stdout, r.stderr);
        assert_eq!(pins(&d), [line.clone()], "{name}: the pin line");
        let said = w.added.replace("{}", &spelled(name, bare));
        assert!(r.stdout.contains(&said), "{name}: pin says it added the line it wrote\n{}", r.stdout);
        passes(&d, w, name);
        // The same, pinned by the line the check gives as the fix.
        let t = TempDir::new("pin-round-trip-fix");
        let d = spec_project(&t, w, name);
        assert_eq!(write_the_fixes(&d, w), [line.trim().to_string()], "{name}: the fix of E102");
        passes(&d, w, &format!("{name} (the fix)"));
    }
}

#[test]
fn a_pin_line_under_an_openspec_spec_is_read_by_the_check_after_it() {
    spec_names(&EN, ENGLISH);
}

#[test]
fn a_pin_line_under_an_openspec_spec_is_read_by_the_check_after_it_in_japanese() {
    spec_names(&JA, JAPANESE);
}

/// A law and the copies of its articles: each `(copy, bytes)` under its directory.
struct Law {
    source: &'static str,
    declared: &'static str,
    dir: &'static str,
    /// The articles cited, as the `.req` writes them, and as the pin line writes them.
    cited: &'static [Spelled],
    copies: &'static [(&'static str, Copy)],
}

/// Where a copy's bytes come from.
enum Copy {
    Fixture(&'static str),
    Text(&'static str),
}

const ECFR: Law = Law {
    source: "cfr",
    declared: "law ecfr \"37 CFR 1\" asof 2026-01-01",
    dir: "sources/law/37-CFR-1@2026-01-01",
    cited: &[("§1.7", false), ("1.6", false)],
    copies: &[("1.7.xml", Copy::Fixture("period_of_months/sources/law/37-CFR-1@2026-01-01/1.7.xml")), ("1.6.xml", Copy::Fixture("period_of_months/sources/law/37-CFR-1@2026-01-01/1.6.xml"))],
};

const SUPPL: &str = "<Article Num=\"3\"><ArticleTitle>第三条</ArticleTitle><Paragraph Num=\"1\"><ParagraphNum/><ParagraphSentence><Sentence Num=\"1\">この法律は、公布の日から施行する。</Sentence></ParagraphSentence></Paragraph></Article>\n";

const EGOV: Law = Law {
    source: "民法",
    declared: "law \"129AC0000000089\" asof 2026-10-01",
    dir: "sources/law/129AC0000000089@2026-10-01",
    cited: &[("第142条", true), ("別表第一", true), ("附則第3条", true), ("附則（令和七年三月三一日法律第一三号）第3条", false)],
    copies: &[
        ("MainProvision-Article_142.xml", Copy::Fixture("period/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml")),
        ("AppdxTable_1.xml", Copy::Text("<AppdxTable Num=\"1\"><AppdxTableTitle>別表第一</AppdxTableTitle><TableStruct><Table><TableRow><TableColumn><Sentence>一の表</Sentence></TableColumn></TableRow></Table></TableStruct></AppdxTable>\n")),
        ("SupplProvision-Article_3.xml", Copy::Text(SUPPL)),
        ("SupplProvision_令和七年三月三一日法律第一三号-Article_3.xml", Copy::Text(SUPPL)),
    ],
};

fn law_project(t: &TempDir, w: &Words, law: &Law) -> (PathBuf, Vec<String>) {
    let d = t.path().join("p");
    std::fs::create_dir_all(d.join(law.dir)).unwrap();
    let mut hashes = Vec::new();
    for (file, copy) in law.copies {
        let bytes = match copy {
            Copy::Fixture(p) => std::fs::read(Path::new("tests/fixtures").join(p)).unwrap(),
            Copy::Text(s) => s.as_bytes().to_vec(),
        };
        hashes.push(ritsu_base::sha256::short(&bytes));
        std::fs::write(d.join(law.dir).join(file), bytes).unwrap();
    }
    let cited: Vec<String> = law.cited.iter().map(|(a, bare)| spelled(a, *bare)).collect();
    std::fs::write(d.join("s.req"), req(w, law.source, law.declared, &cited.join(", "))).unwrap();
    let lines = law.cited.iter().zip(&hashes).map(|((a, bare), h)| format!("  {} sha256:{h}", spelled(a, *bare))).collect();
    (d, lines)
}

fn law_articles(w: &Words, law: &Law) {
    let t = TempDir::new("pin-round-trip-law");
    let (d, lines) = law_project(&t, w, law);
    let r = on(&d, w, &["source", "pin"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(pins(&d), lines, "the pin lines");
    passes(&d, w, law.source);
    let t = TempDir::new("pin-round-trip-law-fix");
    let (d, lines) = law_project(&t, w, law);
    let trimmed: Vec<String> = lines.iter().map(|l| l.trim().to_string()).collect();
    assert_eq!(write_the_fixes(&d, w), trimmed, "the fixes of E102");
    passes(&d, w, &format!("{} (the fixes)", law.source));
}

#[test]
fn a_pin_line_under_a_law_is_read_by_the_check_after_it() {
    law_articles(&EN, &ECFR);
}

#[test]
fn a_pin_line_under_a_law_is_read_by_the_check_after_it_in_japanese() {
    law_articles(&JA, &EGOV);
}
