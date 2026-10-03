//! What 1.0 promises, held (docs/compatibility.md, DESIGN §15.156).
//!
//! Each list here is what 1.0.0 shipped. A 1.x release may add to what the tool has; these
//! tests fail when something on a list is gone. They are not to be edited to make a removal
//! pass — a removal is a 2.0 — only to write down, at 1.0.0, what 1.0.0 is.

use std::collections::BTreeSet;
use std::process::Command;

/// The words E009 refuses as a name. **Fixed**, not merely kept: a word added to the language
/// later is read where no name can stand, or gives way to a declared name — it never joins
/// this list, since a name that was legal would then be refused.
const RESERVED_AT_1_0: &[&str] = &[
    "after", "allocate", "always", "apply", "by", "carry", "clause", "constraint", "contract_only", "count",
    "default", "define", "derive", "description", "down", "elements", "empty", "enum", "examples", "except",
    "exhausted", "false", "final", "fold", "group", "half_down", "half_even", "half_up", "held", "import",
    "initial", "inputs", "keep_max", "machine", "max", "min", "never", "next", "none", "not",
    "of", "once", "outputs", "over", "overrides", "policy", "range", "result", "round", "rule",
    "scenario", "sequence", "source", "starts_with", "stop", "sum", "table", "take_first", "take_unique", "then",
    "true", "up", "when", "where", "with",
];

/// The words an enum value cannot be, a subset of the above. Fixed for the same reason.
const VALUE_RESERVED_AT_1_0: &[&str] = &[
    "after", "by", "default", "empty", "exhausted", "false", "none", "not", "starts_with", "true",
    "with",
];

/// Every diagnostic code of 1.0. A code may be retired — it stays in the ledger, marked so —
/// but it is never removed and its number never given to anything else.
const CODES_AT_1_0: &[&str] = &[
    "E001", "E002", "E003", "E004", "E005", "E006", "E007", "E008", "E009", "E010", "E011", "E012",
    "E013", "E014", "E015", "E016", "E017", "E018", "E019", "E020", "E021", "E022", "E023", "E024",
    "E025", "E026", "E027", "E028", "E029", "E030", "E031", "E032", "E033", "E034", "E035", "E036",
    "E037", "E038", "E039", "E040", "E041", "E042", "E043", "E044", "E045", "E046", "E047", "E049",
    "E048", "E050", "E051", "E052", "E053", "E054", "E055", "E056", "E057", "E058", "E059", "E060",
    "E061", "E062", "E063", "E064", "E101", "E102", "E103", "E104", "E105", "E106", "E107", "E108",
    "E109", "E110", "E111", "E112", "E113", "E114", "E115", "E116", "E117", "E118", "E120", "E121",
    "E119", "W122", "E122", "W123", "E123", "W124", "E124", "E125", "E126", "E127", "E128", "W125",
    "W126", "W127", "W105", "W110", "W111", "W116", "W119", "W120", "W118", "W117", "W121", "W115",
    "W114",
];

/// Every command of 1.0 and the flags it takes.
const FLAGS_AT_1_0: &[(&str, &[&str])] = &[
    ("check", &["--format", "--show-shadow", "--terse", "--diff-base", "--budget", "--lang", "--help"]),
    ("explain", &["--all", "--format", "--lang", "--help"]),
    ("fmt", &["--check", "--format", "--lang", "--help"]),
    ("gen", &["--out", "--check", "--format", "--lang", "--help"]),
    ("test", &["--format", "--require-all", "--proofs", "--lang", "--help"]),
    ("coverage", &["--format", "--lang", "--help"]),
    ("vectors", &["--out", "--lang", "--help"]),
    ("source", &["--via", "--lang", "--help"]),
    ("doc", &["--out", "--format", "--audience", "--lang", "--help"]),
    ("api", &["--format", "--lang", "--help"]),
    ("graph", &["--format", "--lang", "--help"]),
    ("certificate", &["--format", "--lang", "--help"]),
    ("schema", &["--keys", "--lang", "--help"]),
    ("adapter", &["--template", "--lang", "--help"]),
    ("verify", &["--format", "--adapter", "--lang", "--help"]),
    ("fixtures", &["--manifest", "--fill", "--read-as", "--format", "--lang", "--help"]),
    ("replay", &["--fixtures", "--manifest", "--fill", "--read-as", "--format", "--terse", "--lang", "--help"]),
    ("import", &["--name", "--outputs", "--sheet", "--lang", "--help"]),
    ("mcp", &["--timeout", "--lang", "--help"]),
    ("diff", &["--fixtures", "--manifest", "--fill", "--read-as", "--budget", "--format", "--terse", "--lang", "--help"]),
];

#[test]
fn 予約語は1_0のまま増えも減りもしない() {
    let now: BTreeSet<&str> = rulec::kw::RESERVED.iter().copied().collect();
    let then: BTreeSet<&str> = RESERVED_AT_1_0.iter().copied().collect();
    assert_eq!(now, then, "予約語が変わった。足した語は名前を奪い、1.0 で通った規則を E009 で落とす");
    let now: BTreeSet<&str> = rulec::kw::VALUE_RESERVED.iter().copied().collect();
    let then: BTreeSet<&str> = VALUE_RESERVED_AT_1_0.iter().copied().collect();
    assert_eq!(now, then, "値に使えない語が変わった");
}

#[test]
fn 台帳には1_0の診断コードが残っている() {
    let now: BTreeSet<&str> = rulec::codes::ledger().iter().map(|e| e.code).collect();
    let gone: Vec<&&str> = CODES_AT_1_0.iter().filter(|c| !now.contains(**c)).collect();
    assert!(gone.is_empty(), "1.0 のコードが台帳から消えた: {gone:?}");
}

fn help(args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).args(args).output().expect("rulec を起動できない");
    assert_eq!(o.status.code(), Some(0), "rulec {args:?}");
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn コマンドとフラグは1_0のものが残っている() {
    let all = help(&["--help"]);
    let commands: BTreeSet<&str> =
        all.lines().filter_map(|l| l.strip_prefix("  rulec ")).filter_map(|l| l.split_whitespace().next()).collect();
    for (cmd, flags) in FLAGS_AT_1_0 {
        assert!(commands.contains(cmd), "1.0 のコマンド `{cmd}` が消えた");
        let page = help(&[cmd, "--help"]);
        let have: BTreeSet<&str> = page.lines().filter_map(|l| l.strip_prefix("  --")).filter_map(|l| l.split_whitespace().next()).collect();
        for f in *flags {
            assert!(have.contains(&f[2..]), "1.0 のフラグ `rulec {cmd} {f}` が消えた");
        }
    }
}
