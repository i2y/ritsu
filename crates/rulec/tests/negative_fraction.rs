//! A value between -1 and 0 is written with its sign (2026-10-08).
//!
//! `ritsu_units::Rat` wrote a decimal by taking its whole part and then its decimals, and the
//! whole part of -0.5 is 0: the sign went with it. What a person reads and what an agent applies
//! both carried it. The completeness check named `change = 0.5%` for the input -0.5% that no row
//! takes, and its fix — the row an agent pastes — was `| 0.5% | 10USD |`, a row for inputs the
//! table already covers. `fixtures lint` wrote the range of an output that can be -0.1% as
//! `0.1%..0.1%`.
//!
//! Every case is written twice: first in English, then the Japanese version beside it.

use std::path::Path;
use std::process::Command;
use ritsu_testkit::TempDir;

fn rulec(dir: &Path, lang: &str, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", lang)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A rate input that can go below zero, and a table that covers only zero and above.
struct Swing {
    lang: &'static str,
    rule: &'static str,
    /// The witness and the row to add, as `check` writes them.
    said: [&'static str; 2],
}

const SWING_EN: Swing = Swing {
    lang: "en",
    rule: "rule price_swing v1

inputs
  change : rate[step 0.1%]  range >=-0.5% <=0.5%

outputs
  fee : money[USD]  round down(1USD)

table fees
policy unique
| change | -> fee : money[USD] |
| >=0%   | 10USD               |

examples
| change | -> fee |
| 0%     | 10USD  |
",
    said: ["An input that matches no row: change = -0.5%", "| -0.5% | 10USD |"],
};

const SWING_JA: Swing = Swing {
    lang: "ja",
    rule: "rule 値動き手数料(price_swing) v1

inputs
  変動率(change) : rate[step 0.1%]  range >=-0.5% <=0.5%

outputs
  手数料(fee) : money[円]  round down(1円)

table 手数料表(fees)
policy unique
| 変動率 | -> 手数料 : money[円] |
| >=0%   | 100円                 |

examples
| 変動率 | -> 手数料 |
| 0%     | 100円     |
",
    said: ["当てはまらない例: 変動率 = -0.5%", "| -0.5% | 100円 |"],
};

/// The completeness check names the input below zero that no row takes, and the row to add is a
/// row for that input: in the text a person reads and in the JSON's `fix`, which a tool pastes.
#[test]
fn a_gap_below_zero_is_named_with_its_sign() {
    for s in [&SWING_EN, &SWING_JA] {
        let t = TempDir::new(&format!("negative-fraction-gap-{}", s.lang));
        let d = t.path();
        std::fs::write(d.join("swing.rule"), s.rule).unwrap();
        let (code, out) = rulec(d, s.lang, &["check", "swing.rule"]);
        assert_eq!(code, 1, "{}: {out}", s.lang);
        assert!(out.contains("E101") && out.contains(s.said[0]) && out.contains(&format!("`{}`", s.said[1])), "{}: {out}", s.lang);
        let (_, js) = rulec(d, s.lang, &["check", "swing.rule", "--format", "json"]);
        let gap = js.lines().find(|l| l.contains("\"code\":\"E101\"")).unwrap_or_else(|| panic!("{}: E101 が無い:\n{js}", s.lang));
        let j = rulec::json::parse(gap).unwrap();
        let fix = j.get("fix").and_then(|f| f.get("text")).and_then(|t| t.as_str().map(str::to_string));
        assert_eq!(fix.as_deref(), Some(s.said[1]), "{}: 貼る行の符号: {gap}", s.lang);
    }
}

/// An output that can be below zero: the reserve rate of three tiers, the last one negative.
struct Reserve {
    lang: &'static str,
    rule: &'static str,
    records: &'static str,
    /// `fixtures lint`'s note of what the rule can produce, and `replay`'s witness.
    said: [&'static str; 2],
}

const RESERVE_EN: Reserve = Reserve {
    lang: "en",
    rule: "rule reserve_rate v1

enum tier = basic | macro_addon | policy

inputs
  tier : tier

outputs
  rate : rate[step 0.1%]  round down(0.1%)

table rates
policy unique
| tier        | -> rate : rate[step 0.1%] |
| basic       | 0.1%                      |
| macro_addon | 0%                        |
| policy      | -0.1%                     |

examples
| tier   | -> rate |
| policy | -0.1%   |
",
    records: r#"{"in":{"tier":"policy"},"observed":{"rate":-2}}
"#,
    said: [
        "`observed.rate`: outside what the rule can produce, -0.1%..0.1%",
        "tier=policy → rule rate=-0.1% / observed rate=-0.2%",
    ],
};

const RESERVE_JA: Reserve = Reserve {
    lang: "ja",
    rule: "rule 付利(reserve_rate) v1

enum 残高区分(tier) = 基礎残高(basic) | マクロ加算残高(macro_addon) | 政策金利残高(policy)

inputs
  残高区分(tier) : 残高区分

outputs
  付利(rate) : rate[step 0.1%]  round down(0.1%)

table 付利表(rates)
policy unique
| 残高区分       | -> 付利 : rate[step 0.1%] |
| 基礎残高       | 0.1%                      |
| マクロ加算残高 | 0%                        |
| 政策金利残高   | -0.1%                     |

examples
| 残高区分     | -> 付利 |
| 政策金利残高 | -0.1%   |
",
    records: r#"{"in":{"残高区分":"政策金利残高"},"observed":{"付利":-2}}
"#,
    said: [
        "`observed.付利`: 規則が取りうる範囲 -0.1%..0.1% の外です",
        "残高区分=政策金利残高 → 規則 付利=-0.1% / 観測 付利=-0.2%",
    ],
};

/// What the rule can produce is written from -0.1%, and the two answers of a record are written
/// with their signs.
#[test]
fn a_range_below_zero_is_written_with_its_sign() {
    for r in [&RESERVE_EN, &RESERVE_JA] {
        let t = TempDir::new(&format!("negative-fraction-range-{}", r.lang));
        let d = t.path();
        std::fs::write(d.join("reserve.rule"), r.rule).unwrap();
        std::fs::write(d.join("r.jsonl"), r.records).unwrap();
        let (code, out) = rulec(d, r.lang, &["check", "reserve.rule"]);
        assert_eq!(code, 0, "{}: {out}", r.lang);
        let (_, lint) = rulec(d, r.lang, &["fixtures", "lint", "r.jsonl", "reserve.rule"]);
        assert!(lint.contains(r.said[0]), "{}: {lint}", r.lang);
        let (code, replay) = rulec(d, r.lang, &["replay", "reserve.rule", "--fixtures", "r.jsonl"]);
        assert_eq!(code, 1, "{}: {replay}", r.lang);
        assert!(replay.contains(r.said[1]), "{}: {replay}", r.lang);
    }
}
